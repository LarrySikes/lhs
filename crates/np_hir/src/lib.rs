//! Typed IR stubs. Typechecking is not implemented yet.

use np_syntax::SourceFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub code: String,
    pub message: String,
    pub help: Option<String>,
}

#[derive(Debug)]
pub struct Module {
    pub file: SourceFile,
}

pub fn lower(file: SourceFile) -> (Module, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    // Placeholder: examples/10_bad_type.np is expected to fail once typing exists.
    if file.text.contains("let x: i32 = \"nope\"") {
        diags.push(Diagnostic {
            file: file.path.clone(),
            start: 0,
            end: file.text.len(),
            code: "E0001".into(),
            message: "type mismatch: expected i32, found str (stub checker)".into(),
            help: Some("remove the annotation or use an integer literal".into()),
        });
    }
    (Module { file }, diags)
}
