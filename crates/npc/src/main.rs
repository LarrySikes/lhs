use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use np_codegen::{compile_c_to_binary, emit_c};
use np_eval::run_program_stdout;
use np_hir::{check, Diagnostic};
use np_syntax::format_program;
use serde::Serialize;

#[derive(Serialize)]
struct JsonDiag<'a> {
    file: &'a str,
    span: [usize; 2],
    code: &'a str,
    message: &'a str,
    help: Option<&'a str>,
}

fn usage() -> ! {
    eprintln!(
        "npc — newproj compiler v0.1\n\n\
         Usage:\n\
           npc check [--json] <file.np>\n\
           npc run <file.np>\n\
           npc fmt [--write] <file.np>\n\
           npc test [examples_dir]\n\
           npc build <file.np> [-o outfile]   (simple subset → native via cc)\n\n\
         Compiler is Rust (`npc`). Language name TBD.\n"
    );
    process::exit(2);
}

fn print_diag(d: &Diagnostic, json: bool) {
    if json {
        let j = JsonDiag {
            file: &d.file,
            span: [d.start, d.end],
            code: &d.code,
            message: &d.message,
            help: d.help.as_deref(),
        };
        println!("{}", serde_json::to_string(&j).unwrap());
    } else {
        eprintln!("{}:{}-{}: {} {}", d.file, d.start, d.end, d.code, d.message);
        if let Some(h) = &d.help {
            eprintln!("  help: {h}");
        }
    }
}

fn load_ok(path: &Path) -> Option<np_hir::Module> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("npc: cannot read {}: {e}", path.display());
            return None;
        }
    };
    let (module, diags) = check(&path.display().to_string(), text);
    for d in &diags {
        print_diag(d, false);
    }
    if !diags.is_empty() {
        return None;
    }
    module
}

fn cmd_check(path: &Path, json: bool) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("npc: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let (module, diags) = check(&path.display().to_string(), text);
    for d in &diags {
        print_diag(d, json);
    }
    if diags.is_empty() {
        if !json {
            if let Some(m) = module {
                let n = m.program.items.len();
                eprintln!("npc: ok ({n} item{})", if n == 1 { "" } else { "s" });
            }
        }
        0
    } else {
        1
    }
}

fn cmd_run(path: &Path) -> i32 {
    let Some(module) = load_ok(path) else {
        return 1;
    };
    match run_program_stdout(&module.program) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("npc: runtime error: {e}");
            1
        }
    }
}

fn cmd_fmt(path: &Path, write: bool) -> i32 {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("npc: cannot read {}: {e}", path.display());
            return 1;
        }
    };
    let (module, diags) = check(&path.display().to_string(), text);
    // fmt even with type errors if parse succeeded
    let Some(module) = module else {
        for d in &diags {
            print_diag(d, false);
        }
        return 1;
    };
    let formatted = format_program(&module.program);
    if write {
        if let Err(e) = fs::write(path, formatted) {
            eprintln!("npc: write failed: {e}");
            return 1;
        }
        eprintln!("npc: wrote {}", path.display());
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
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("np"))
            .collect(),
        Err(e) => {
            eprintln!("npc: cannot read {}: {e}", dir.display());
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
        let (module, diags) = check(&path.display().to_string(), text);
        let check_ok = diags.is_empty() && module.is_some();
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
    eprintln!("npc test: {pass} passed, {fail} failed");
    if fail == 0 {
        0
    } else {
        1
    }
}

fn cmd_build(path: &Path, out: &Path) -> i32 {
    let Some(module) = load_ok(path) else {
        return 1;
    };
    match emit_c(&module.program) {
        Ok(c) => match compile_c_to_binary(&c, &out.display().to_string()) {
            Ok(()) => {
                eprintln!("npc: built {}", out.display());
                0
            }
            Err(e) => {
                eprintln!("npc: link failed: {}", e.message);
                1
            }
        },
        Err(e) => {
            eprintln!("npc: build: {}", e.message);
            eprintln!("npc: tip: use `npc run {}` for full language support", path.display());
            1
        }
    }
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
                    eprintln!("npc: unknown flag {a}");
                    usage();
                } else {
                    file = Some(a);
                }
            }
            let Some(file) = file else { usage() };
            process::exit(cmd_check(Path::new(&file), json));
        }
        "run" => {
            let Some(file) = args.into_iter().next() else {
                usage();
            };
            process::exit(cmd_run(Path::new(&file)));
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
        "build" => {
            let mut out: Option<String> = None;
            let mut file: Option<String> = None;
            let mut it = args.into_iter();
            while let Some(a) = it.next() {
                if a == "-o" {
                    out = it.next();
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
            process::exit(cmd_build(Path::new(&file), Path::new(&out)));
        }
        "-h" | "--help" | "help" => usage(),
        other => {
            eprintln!("npc: unknown command `{other}`");
            usage();
        }
    }
}
