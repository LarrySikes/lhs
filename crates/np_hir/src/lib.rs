//! Lowering / early checks. Full typechecker comes next.

use np_syntax::{parse_file, Diagnostic as SynDiag, Expr, Item, Program, SourceFile, Stmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub code: String,
    pub message: String,
    pub help: Option<String>,
}

impl From<SynDiag> for Diagnostic {
    fn from(d: SynDiag) -> Self {
        Self {
            file: d.file,
            start: d.span.start,
            end: d.span.end,
            code: d.code,
            message: d.message,
            help: d.help,
        }
    }
}

#[derive(Debug)]
pub struct Module {
    pub file: SourceFile,
    pub program: Program,
}

pub fn check(path: &str, text: String) -> (Option<Module>, Vec<Diagnostic>) {
    match parse_file(path, text) {
        Ok((file, program)) => {
            let diags = stub_type_checks(path, &program);
            let module = Module { file, program };
            (Some(module), diags)
        }
        Err(e) => (None, vec![Diagnostic::from(e)]),
    }
}

/// Minimal stub until a real typechecker exists: catch the deliberate
/// `examples/10_bad_type.np` mismatch and a few obvious let-annotation errors.
fn stub_type_checks(path: &str, program: &Program) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for item in &program.items {
        let Item::Fn(f) = item;
        for stmt in &f.body.stmts {
            if let Stmt::Let {
                name: _,
                ty: Some(ty),
                init,
                span,
            } = stmt
            {
                if ty.name == "i32" {
                    if let Expr::Str { span: lit_span, .. } = init {
                        diags.push(Diagnostic {
                            file: path.to_string(),
                            start: lit_span.start,
                            end: lit_span.end,
                            code: "E0001".into(),
                            message: "type mismatch: expected i32, found str".into(),
                            help: Some("use an integer literal, e.g. 0".into()),
                        });
                        let _ = span;
                    }
                }
            }
        }
    }
    diags
}
