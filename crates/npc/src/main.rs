// Author: Larry Sikes
// Last modified: 2026-11-01
//
// lhsc — LHS compiler CLI (check / run / build / py-check / …)

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
           lhsc build <file.lhs> [-o outfile] [--emit=c|cranelift]\n\
           lhsc py-check <dir|file.py>     # scan a Python project for LHS gaps\n\n\
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
        "http_get",
        "http_post",
        "http_request",
        "json_get",
        "re_find",
        "re_replace",
        "db_exec",
        "db_query",
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

/// Map a top-level Python import to an LHS note.
fn py_import_status(name: &str) -> (&'static str, &'static str) {
    match name {
        "json" => ("partial", "use json_get(doc, \"dotted.path\")"),
        "urllib" | "urllib.request" | "requests" | "http" | "httpx" => {
            (
                "partial",
                "http_get / http_post / http_request — needs curl on PATH",
            )
        }
        "math" => ("ok", "use math"),
        "os" => ("partial", "getenv / argc / arg cover a small slice"),
        "sys" => ("partial", "argc / arg / exit"),
        "time" => ("partial", "now_ms / sleep_ms"),
        "typing" | "dataclasses" | "enum" | "abc" => ("rewrite", "use LHS types / structs / ADTs"),
        "flask" | "django" | "fastapi" | "bottle" | "tornado" => {
            ("gap", "no web framework — rewrite as CLI or wait for HTTP server")
        }
        "tkinter" | "PyQt5" | "PyQt6" | "wx" | "gi" => {
            ("gap", "no GUI — outside LHS v1 goals")
        }
        "numpy" | "pandas" | "scipy" | "sklearn" | "torch" | "tensorflow" => {
            ("gap", "no numeric stack yet — rewrite hot loops in LHS or call C")
        }
        "sqlite3" => ("partial", "db_exec / db_query — needs sqlite3 on PATH"),
        "sqlalchemy" | "psycopg2" => ("gap", "no ORM / Postgres API yet — use db_* for SQLite"),
        "re" => ("partial", "re_find / re_replace"),
        "pathlib" | "shutil" | "glob" => ("partial", "read_file / write_file only"),
        "threading" | "multiprocessing" | "asyncio" => {
            ("partial", "task / await — structured, not a drop-in")
        }
        "subprocess" | "socket" => ("gap", "not exposed; http_get covers simple GET"),
        _ => ("unknown", "no mapping — plan a manual port"),
    }
}

fn collect_py_files(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if root.extension().and_then(|e| e.to_str()) == Some("py") {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(rd) = fs::read_dir(root) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "__pycache__" || name == "env" || name == "venv"
            || name == ".venv"
        {
            continue;
        }
        if p.is_dir() {
            collect_py_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("py") {
            out.push(p);
        }
    }
}

fn extract_py_imports(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if let Some(rest) = t.strip_prefix("import ") {
            for part in rest.split(',') {
                let name = part
                    .trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        } else if let Some(rest) = t.strip_prefix("from ") {
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches('.')
                .split('.')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn cmd_py_check(target: &Path) -> i32 {
    if !target.exists() {
        eprintln!("lhsc py-check: not found: {}", target.display());
        return 1;
    }
    let mut files = Vec::new();
    collect_py_files(target, &mut files);
    files.sort();
    if files.is_empty() {
        eprintln!("lhsc py-check: no .py files under {}", target.display());
        return 1;
    }
    println!("LHS Python project check");
    println!("Target: {}", target.display());
    println!("Python files: {}", files.len());
    println!();
    println!("For LHS to be truly useful, it should handle most existing Python");
    println!("projects. This report shows what this tree needs vs what LHS has.");
    println!();

    let mut all_imports: Vec<String> = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).unwrap_or_default();
        for imp in extract_py_imports(&text) {
            all_imports.push(imp);
        }
    }
    all_imports.sort();
    all_imports.dedup();

    let mut ok = 0;
    let mut partial = 0;
    let mut gap = 0;
    let mut unknown = 0;
    let mut rewrite = 0;
    println!("Imports:");
    for imp in &all_imports {
        let (status, note) = py_import_status(imp);
        match status {
            "ok" => ok += 1,
            "partial" => partial += 1,
            "gap" => gap += 1,
            "rewrite" => rewrite += 1,
            _ => unknown += 1,
        }
        println!("  [{status:7}] {imp:16}  {note}");
    }
    println!();
    println!(
        "Summary: {} ok, {} partial, {} rewrite, {} gap, {} unknown",
        ok, partial, rewrite, gap, unknown
    );
    println!();
    println!("Builtins: http_get/post/request, json_get, re_find/replace,");
    println!("db_exec/db_query, read_file/write_file, getenv, argc/arg.");
    println!("Demo: apps/weather.lhs  Guide: docs/PYTHON.md");
    if gap > 0 || unknown > 0 {
        println!();
        println!("Result: not portable as-is — close gaps or rewrite those parts.");
        2
    } else if partial > 0 || rewrite > 0 {
        println!();
        println!("Result: possible with a manual port of partial/rewrite imports.");
        0
    } else {
        println!();
        println!("Result: imports look covered — try a manual .lhs port.");
        0
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
                } else if file.is_none() {
                    file = Some(a);
                }
                // Further args stay in process argv for argc()/arg().
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
        "py-check" => {
            let Some(target) = args.into_iter().next() else {
                usage();
            };
            process::exit(cmd_py_check(Path::new(&target)));
        }
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
