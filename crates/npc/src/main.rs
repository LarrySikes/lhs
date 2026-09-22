use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use lhs_jit::{build_aot, is_jit_supported, run_jit};
use np_codegen::{build_native, compile_c_to_binary, emit_c};
use np_eval::run_program_stdout;
use np_hir::{check_with_imports, Diagnostic};
use np_syntax::format_program;
use serde::Serialize;

#[derive(Serialize)]
struct JsonDiag<'a> {
    file: &'a str,
    span: [usize; 2],
    code: &'a str,
    message: &'a str,
    help: Option<&'a str>,
    warning: bool,
}

fn usage() -> ! {
    eprintln!(
        "lhsc — LHS compiler v0.6\n\n\
         Usage:\n\
           lhsc check [--json] <file.lhs>\n\
           lhsc run [--jit] <file.lhs>\n\
           lhsc watch [--jit] <file.lhs>   # re-run on file change\n\
           lhsc fmt [--write] <file.lhs>\n\
           lhsc test [examples_dir]\n\
           lhsc lib                        # list stdlib modules + builtins\n\
           lhsc build <file.lhs> [-o outfile] [--emit=c|cranelift]\n\n\
         Modules: `use math` loads stdlib/math.lhs (see docs/PACKAGES.md).\n\
         Language: LHS. Compiler implemented in Rust.\n"
    );
    process::exit(2);
}

fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.is_error())
}

fn workspace_root() -> PathBuf {
    // crates/npc -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn search_dirs_for(file: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(extra) = env::var("LHS_PATH") {
        for part in extra.split(':') {
            if !part.is_empty() {
                dirs.push(PathBuf::from(part));
            }
        }
    }
    let root = workspace_root();
    dirs.push(root.join("stdlib"));
    if let Some(parent) = file.parent() {
        dirs.push(parent.to_path_buf());
    }
    dirs.push(root);
    dirs
}

fn print_diag(d: &Diagnostic, json: bool) {
    if json {
        let j = JsonDiag {
            file: &d.file,
            span: [d.start, d.end],
            code: &d.code,
            message: &d.message,
            help: d.help.as_deref(),
            warning: d.warning,
        };
        println!("{}", serde_json::to_string(&j).unwrap());
    } else {
        let kind = if d.warning { "warning" } else { "error" };
        eprintln!(
            "{}:{}-{}: {} {} {}",
            d.file, d.start, d.end, kind, d.code, d.message
        );
        if let Some(h) = &d.help {
            eprintln!("  help: {h}");
        }
    }
}

fn load_ok(path: &Path) -> Option<np_hir::Module> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lhsc: cannot read {}: {e}", path.display());
            return None;
        }
    };
    let dirs = search_dirs_for(path);
    let (module, diags) = check_with_imports(&path.display().to_string(), text, &dirs);
    for d in &diags {
        print_diag(d, false);
    }
    if has_errors(&diags) {
        return None;
    }
    module
}

fn cmd_check(path: &Path, json: bool) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lhsc: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let dirs = search_dirs_for(path);
    let (module, diags) = check_with_imports(&path.display().to_string(), text, &dirs);
    for d in &diags {
        print_diag(d, json);
    }
    if has_errors(&diags) {
        1
    } else {
        if !json {
            if let Some(m) = module {
                let n = m.program.items.len();
                let w = diags.iter().filter(|d| d.warning).count();
                if w > 0 {
                    eprintln!(
                        "lhsc: ok ({n} item{}, {w} warning{})",
                        if n == 1 { "" } else { "s" },
                        if w == 1 { "" } else { "s" }
                    );
                } else {
                    eprintln!("lhsc: ok ({n} item{})", if n == 1 { "" } else { "s" });
                }
            }
        }
        0
    }
}

fn cmd_lib() -> i32 {
    println!("LHS builtins:");
    for name in [
        "print",
        "eprint",
        "read_file",
        "write_file",
        "read_line",
        "str_slice",
        "find_char",
        "abs",
        "min",
        "max",
        "assert",
        "getenv",
        "argc",
        "arg",
        "exit",
        "sleep_ms",
        "now_ms",
        "gc",
    ] {
        println!("  {name}");
    }
    println!("\nstdlib modules (use <name>):");
    let dir = workspace_root().join("stdlib");
    let mut mods = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) == Some("lhs") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    mods.push(stem.to_string());
                }
            }
        }
    }
    mods.sort();
    if mods.is_empty() {
        println!("  (none found in {})", dir.display());
    } else {
        for m in mods {
            println!("  {m}");
        }
    }
    println!("\nSearch path: LHS_PATH + {} + <source dir>", dir.display());
    0
}

fn cmd_run(path: &Path, use_jit: bool) -> i32 {
    let Some(module) = load_ok(path) else {
        return 1;
    };
    if use_jit {
        if !is_jit_supported(&module.program) {
            eprintln!(
                "lhsc: --jit: unsupported construct for Cranelift; use plain `lhsc run` or embed build."
            );
            return 1;
        }
        return match run_jit(&module.program) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("lhsc: jit error: {}", e.message);
                1
            }
        };
    }
    match run_program_stdout(&module.program) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("lhsc: runtime error: {e}");
            1
        }
    }
}

fn cmd_fmt(path: &Path, write: bool) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lhsc: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let program = match np_syntax::parse_file(&path.display().to_string(), text) {
        Ok((_, p)) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let formatted = format_program(&program);
    if write {
        if let Err(e) = fs::write(path, formatted) {
            eprintln!("lhsc: write failed: {e}");
            return 1;
        }
        eprintln!("lhsc: wrote {}", path.display());
    } else {
        print!("{formatted}");
    }
    0
}

fn cmd_test(dir: &Path) -> i32 {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("lhs") | Some("np")
                )
            })
            .collect(),
        Err(e) => {
            eprintln!("lhsc: cannot read {}: {e}", dir.display());
            return 1;
        }
    };
    entries.sort();
    let mut pass = 0;
    let mut fail = 0;
    for path in &entries {
        let name = path.file_name().unwrap().to_string_lossy();
        let expect_fail = name.contains("bad_type") || name.starts_with("10_");
        let text = fs::read_to_string(path).unwrap_or_default();
        let dirs = search_dirs_for(path);
        let (module, diags) = check_with_imports(&path.display().to_string(), text, &dirs);
        let check_ok = !has_errors(&diags) && module.is_some();
        if expect_fail {
            if !check_ok {
                println!("ok   {name} (expected type/check failure)");
                pass += 1;
            } else {
                println!("FAIL {name} (expected failure, got ok)");
                fail += 1;
            }
            continue;
        }
        if !check_ok {
            println!("FAIL {name} (check)");
            for d in &diags {
                println!("       {d}");
            }
            fail += 1;
            continue;
        }
        let module = module.unwrap();
        match run_program_stdout(&module.program) {
            Ok(_) => {
                println!("ok   {name}");
                pass += 1;
            }
            Err(e) => {
                println!("FAIL {name} (runtime: {e})");
                fail += 1;
            }
        }
    }
    eprintln!("lhsc test: {pass} passed, {fail} failed");
    if fail == 0 {
        0
    } else {
        1
    }
}

fn cmd_build(path: &Path, out: &Path, emit: EmitMode) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lhsc: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let dirs = search_dirs_for(path);
    let (module, diags) = check_with_imports(&path.display().to_string(), text.clone(), &dirs);
    for d in &diags {
        print_diag(d, false);
    }
    if has_errors(&diags) || module.is_none() {
        return 1;
    }
    let module = module.unwrap();

    match emit {
        EmitMode::C => match emit_c(&module.program) {
            Ok(c) => match compile_c_to_binary(&c, &out.display().to_string()) {
                Ok(()) => {
                    eprintln!("lhsc: built {} (--emit=c subset)", out.display());
                    0
                }
                Err(e) => {
                    eprintln!("lhsc: link failed: {}", e.message);
                    1
                }
            },
            Err(e) => {
                eprintln!("lhsc: --emit=c: {}", e.message);
                1
            }
        },
        EmitMode::Cranelift => {
            if !is_jit_supported(&module.program) {
                eprintln!(
                    "lhsc: --emit=cranelift: outside subset; use default `lhsc build` for full language"
                );
                return 1;
            }
            match build_aot(&module.program, &out.display().to_string()) {
                Ok(()) => {
                    eprintln!("lhsc: built {} (--emit=cranelift)", out.display());
                    0
                }
                Err(e) => {
                    eprintln!("lhsc: cranelift build failed: {}", e.message);
                    1
                }
            }
        }
        EmitMode::Embed => {
            let merged = format_program(&module.program);
            match build_native(&merged, &out.display().to_string()) {
                Ok(()) => {
                    eprintln!("lhsc: built {}", out.display());
                    0
                }
                Err(e) => {
                    eprintln!("lhsc: build failed: {}", e.message);
                    1
                }
            }
        }
    }
}

fn cmd_watch(path: &Path, use_jit: bool) -> i32 {
    use std::time::{Duration, SystemTime};
    eprintln!(
        "lhsc watch: {} (Ctrl-C to stop){}",
        path.display(),
        if use_jit { " [--jit]" } else { "" }
    );
    let mut last: Option<SystemTime> = None;
    loop {
        let modified = match fs::metadata(path).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("lhsc watch: cannot stat {}: {e}", path.display());
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
        };
        if last.map(|t| t != modified).unwrap_or(true) {
            last = Some(modified);
            eprintln!("---- lhsc watch: run {} ----", path.display());
            let code = cmd_run(path, use_jit);
            if code != 0 {
                eprintln!("lhsc watch: exit {code}");
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[derive(Clone, Copy)]
enum EmitMode {
    Embed,
    C,
    Cranelift,
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "check" => {
            let mut json = false;
            let mut file: Option<String> = None;
            for a in args {
                if a == "--json" {
                    json = true;
                } else if a.starts_with('-') {
                    eprintln!("lhsc: unknown flag {a}");
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            process::exit(cmd_check(Path::new(&file), json));
        }
        "run" => {
            let mut use_jit = false;
            let mut file: Option<String> = None;
            for a in args {
                if a == "--jit" {
                    use_jit = true;
                } else if a.starts_with('-') {
                    eprintln!("lhsc: unknown flag {a}");
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            process::exit(cmd_run(Path::new(&file), use_jit));
        }
        "watch" => {
            let mut use_jit = false;
            let mut file: Option<String> = None;
            for a in args {
                if a == "--jit" {
                    use_jit = true;
                } else if a.starts_with('-') {
                    eprintln!("lhsc: unknown flag {a}");
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            process::exit(cmd_watch(Path::new(&file), use_jit));
        }
        "fmt" => {
            let mut write = false;
            let mut file: Option<String> = None;
            for a in args {
                if a == "--write" {
                    write = true;
                } else if a.starts_with('-') {
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            process::exit(cmd_fmt(Path::new(&file), write));
        }
        "test" => {
            let dir = args
                .into_iter()
                .next()
                .unwrap_or_else(|| "examples".into());
            process::exit(cmd_test(Path::new(&dir)));
        }
        "lib" => process::exit(cmd_lib()),
        "build" => {
            let mut out: Option<String> = None;
            let mut file: Option<String> = None;
            let mut emit = EmitMode::Embed;
            let mut it = args.into_iter();
            while let Some(a) = it.next() {
                if a == "-o" {
                    out = it.next();
                } else if a == "--emit=c" {
                    emit = EmitMode::C;
                } else if a == "--emit=cranelift" {
                    emit = EmitMode::Cranelift;
                } else if a.starts_with('-') {
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            let out = out.unwrap_or_else(|| {
                Path::new(&file)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            });
            process::exit(cmd_build(Path::new(&file), Path::new(&out), emit));
        }
        "-h" | "--help" | "help" => usage(),
        other => {
            eprintln!("lhsc: unknown command `{other}`");
            usage();
        }
    }
}
