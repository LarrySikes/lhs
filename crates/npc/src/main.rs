use std::env;
use std::fs;
use std::path::Path;
use std::process;

use np_hir::{check, Diagnostic};
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
        "npc — newproj compiler\n\n\
         Usage:\n\
           npc check [--json] <file.np>\n\
           npc run <file.np>     (not implemented)\n\
           npc fmt <file.np>     (not implemented)\n\
           npc test              (run `cargo test` in the workspace)\n\n\
         The compiler binary is `npc`. The language name is still TBD.\n"
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
            } else {
                eprintln!("npc: ok");
            }
        }
        0
    } else {
        1
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
        "run" | "fmt" => {
            eprintln!("npc: `{cmd}` not implemented yet");
            process::exit(1);
        }
        "test" => {
            eprintln!("npc: run `cargo test` from the workspace root");
            process::exit(2);
        }
        "-h" | "--help" | "help" => usage(),
        other => {
            eprintln!("npc: unknown command `{other}`");
            usage();
        }
    }
}
