//! Lexer / parser stubs. Real grammar lands after examples stabilize.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    NotImplemented { message: String },
}

pub fn parse_file(path: &str, text: String) -> Result<SourceFile, ParseError> {
    // MVP: accept any text as an opaque source unit; no AST yet.
    if text.is_empty() {
        return Err(ParseError::NotImplemented {
            message: "empty source".into(),
        });
    }
    Ok(SourceFile {
        path: path.to_string(),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(parse_file("x.np", String::new()).is_err());
    }
}
