//! Lexer and parser for `.np` source. Spec = `examples/`.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // keywords
    Fn,
    Let,
    If,
    Else,
    Return,
    Match,
    Type,
    Extern,
    Unsafe,
    Task,
    Await,
    // literals / idents
    Ident(String),
    Int(i64),
    Float(String),
    Str(String),
    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semi,
    Dot,
    Eq,
    Arrow, // ->
    FatArrow, // =>
    Plus,
    Minus,
    Star,
    Slash,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Pipe,
    Amp,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub span: Span,
    pub code: String,
    pub message: String,
    pub help: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}: {} {}",
            self.file, self.span.start, self.span.end, self.code, self.message
        )
    }
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    file: String,
}

impl<'a> Lexer<'a> {
    pub fn new(file: &str, src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            file: file.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.bump();
            }
            if self.peek() == Some(b'#') {
                // line comment
                while let Some(c) = self.bump() {
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }
            if self.peek() == Some(b'/') && self.bytes.get(self.pos + 1) == Some(&b'/') {
                self.bump();
                self.bump();
                while let Some(c) = self.bump() {
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    pub fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_ws_and_comments();
        let start = self.pos;
        let Some(c) = self.bump() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span::new(start, start),
            });
        };

        let kind = match c {
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            b';' => TokenKind::Semi,
            b'.' => TokenKind::Dot,
            b'+' => TokenKind::Plus,
            b'*' => TokenKind::Star,
            b'/' => TokenKind::Slash,
            b'|' => {
                if self.peek() == Some(b'|') {
                    self.bump();
                    TokenKind::OrOr
                } else {
                    TokenKind::Pipe
                }
            }
            b'&' => {
                if self.peek() == Some(b'&') {
                    self.bump();
                    TokenKind::AndAnd
                } else {
                    TokenKind::Amp
                }
            }
            b'=' => {
                if self.peek() == Some(b'>') {
                    self.bump();
                    TokenKind::FatArrow
                } else if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            b'!' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::Ne
                } else {
                    return Err(self.err(start, self.pos, "E0002", "unexpected '!'"));
                }
            }
            b'<' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            b'>' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            b'-' => {
                if self.peek() == Some(b'>') {
                    self.bump();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            b'"' => {
                let mut s = String::new();
                loop {
                    match self.bump() {
                        Some(b'"') => break,
                        Some(b'\\') => match self.bump() {
                            Some(b'n') => s.push('\n'),
                            Some(b't') => s.push('\t'),
                            Some(b'"') => s.push('"'),
                            Some(b'\\') => s.push('\\'),
                            Some(o) => s.push(o as char),
                            None => {
                                return Err(self.err(
                                    start,
                                    self.pos,
                                    "E0003",
                                    "unterminated string escape",
                                ))
                            }
                        },
                        Some(ch) => s.push(ch as char),
                        None => {
                            return Err(self.err(start, self.pos, "E0003", "unterminated string"))
                        }
                    }
                }
                TokenKind::Str(s)
            }
            b'0'..=b'9' => {
                let mut end = self.pos;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump();
                    end = self.pos;
                }
                if self.peek() == Some(b'.')
                    && matches!(self.bytes.get(self.pos + 1), Some(b'0'..=b'9'))
                {
                    self.bump();
                    while matches!(self.peek(), Some(b'0'..=b'9')) {
                        self.bump();
                    }
                    TokenKind::Float(self.src[start..self.pos].to_string())
                } else {
                    let text = &self.src[start..end];
                    let n: i64 = text.parse().map_err(|_| {
                        self.err(start, end, "E0004", &format!("invalid integer `{text}`"))
                    })?;
                    TokenKind::Int(n)
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while matches!(
                    self.peek(),
                    Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                ) {
                    self.bump();
                }
                let text = &self.src[start..self.pos];
                TokenKind::Ident(text.to_string()).into_keyword()
            }
            _ => {
                return Err(self.err(
                    start,
                    self.pos,
                    "E0002",
                    &format!("unexpected character {:?}", c as char),
                ))
            }
        };

        Ok(Token {
            kind,
            span: Span::new(start, self.pos),
        })
    }

    fn err(&self, start: usize, end: usize, code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            file: self.file.clone(),
            span: Span::new(start, end),
            code: code.into(),
            message: message.into(),
            help: None,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token()?;
            let done = t.kind == TokenKind::Eof;
            out.push(t);
            if done {
                break;
            }
        }
        Ok(out)
    }
}

impl TokenKind {
    fn into_keyword(self) -> Self {
        match self {
            TokenKind::Ident(ref s) => match s.as_str() {
                "fn" => TokenKind::Fn,
                "let" => TokenKind::Let,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "return" => TokenKind::Return,
                "match" => TokenKind::Match,
                "type" => TokenKind::Type,
                "extern" => TokenKind::Extern,
                "unsafe" => TokenKind::Unsafe,
                "task" => TokenKind::Task,
                "await" => TokenKind::Await,
                _ => self,
            },
            other => other,
        }
    }
}

// --- AST ---

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Fn(FnItem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnItem {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub ret: Option<TypeRef>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<TypeRef>,
        init: Expr,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Ident { name: String, span: Span },
    Int { value: i64, span: Span },
    Float { text: String, span: Span },
    Str { value: String, span: Span },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Group {
        inner: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

pub struct Parser {
    tokens: Vec<Token>,
    idx: usize,
    file: String,
}

impl Parser {
    pub fn new(file: &str, tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            idx: 0,
            file: file.to_string(),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.idx)
            .unwrap_or_else(|| self.tokens.last().expect("eof token"))
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.idx + 1 < self.tokens.len() {
            self.idx += 1;
        }
        t
    }

    fn expect_punct(&mut self, want: TokenKind) -> Result<Token, Diagnostic> {
        let t = self.bump();
        if t.kind == want {
            Ok(t)
        } else {
            Err(self.diag(
                t.span,
                "E0010",
                &format!("expected {:?}, found {:?}", want, t.kind),
            ))
        }
    }

    fn diag(&self, span: Span, code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            file: self.file.clone(),
            span,
            code: code.into(),
            message: message.into(),
            help: None,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        while self.peek().kind != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        match self.peek().kind {
            TokenKind::Fn => Ok(Item::Fn(self.parse_fn()?)),
            _ => {
                let t = self.peek().clone();
                Err(self.diag(t.span, "E0011", "expected `fn` item"))
            }
        }
    }

    fn parse_fn(&mut self) -> Result<FnItem, Diagnostic> {
        let start = self.expect_punct(TokenKind::Fn)?.span.start;
        let name_tok = self.bump();
        let (name, name_span) = match name_tok.kind {
            TokenKind::Ident(s) => (s, name_tok.span),
            _ => {
                return Err(self.diag(name_tok.span, "E0012", "expected function name"));
            }
        };
        self.expect_punct(TokenKind::LParen)?;
        let mut params = Vec::new();
        if self.peek().kind != TokenKind::RParen {
            loop {
                params.push(self.parse_param()?);
                if self.peek().kind == TokenKind::Comma {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_punct(TokenKind::RParen)?;
        let ret = if self.peek().kind == TokenKind::Arrow {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = Span::new(start, body.span.end);
        Ok(FnItem {
            name,
            name_span,
            params,
            ret,
            body,
            span,
        })
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let name_tok = self.bump();
        let (name, start) = match name_tok.kind {
            TokenKind::Ident(s) => (s, name_tok.span.start),
            _ => return Err(self.diag(name_tok.span, "E0013", "expected parameter name")),
        };
        self.expect_punct(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        let span = Span::new(start, ty.span.end);
        Ok(Param { name, ty, span })
    }

    fn parse_type(&mut self) -> Result<TypeRef, Diagnostic> {
        let t = self.bump();
        match t.kind {
            TokenKind::Ident(name) => Ok(TypeRef {
                name,
                span: t.span,
            }),
            _ => Err(self.diag(t.span, "E0014", "expected type name")),
        }
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect_punct(TokenKind::LBrace)?.span.start;
        let mut stmts = Vec::new();
        while self.peek().kind != TokenKind::RBrace && self.peek().kind != TokenKind::Eof {
            stmts.push(self.parse_stmt()?);
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(Block {
            stmts,
            span: Span::new(start, end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match self.peek().kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::Return => {
                let start = self.bump().span.start;
                let value = if matches!(
                    self.peek().kind,
                    TokenKind::RBrace | TokenKind::Semi | TokenKind::Eof
                ) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                let end = value
                    .as_ref()
                    .map(expr_span)
                    .map(|s| s.end)
                    .unwrap_or(start + 6);
                if self.peek().kind == TokenKind::Semi {
                    self.bump();
                }
                Ok(Stmt::Return {
                    value,
                    span: Span::new(start, end),
                })
            }
            _ => {
                let e = self.parse_expr()?;
                if self.peek().kind == TokenKind::Semi {
                    self.bump();
                }
                Ok(Stmt::Expr(e))
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect_punct(TokenKind::Let)?.span.start;
        let name_tok = self.bump();
        let name = match name_tok.kind {
            TokenKind::Ident(s) => s,
            _ => return Err(self.diag(name_tok.span, "E0015", "expected binding name")),
        };
        let ty = if self.peek().kind == TokenKind::Colon {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_punct(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        let end = expr_span(&init).end;
        if self.peek().kind == TokenKind::Semi {
            self.bump();
        }
        Ok(Stmt::Let {
            name,
            ty,
            init,
            span: Span::new(start, end),
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_and()?;
        while self.peek().kind == TokenKind::OrOr {
            self.bump();
            let rhs = self.parse_and()?;
            let span = Span::new(expr_span(&lhs).start, expr_span(&rhs).end);
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_cmp()?;
        while self.peek().kind == TokenKind::AndAnd {
            self.bump();
            let rhs = self.parse_cmp()?;
            let span = Span::new(expr_span(&lhs).start, expr_span(&rhs).end);
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_add()?;
            let span = Span::new(expr_span(&lhs).start, expr_span(&rhs).end);
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul()?;
            let span = Span::new(expr_span(&lhs).start, expr_span(&rhs).end);
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_postfix()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_postfix()?;
            let span = Span::new(expr_span(&lhs).start, expr_span(&rhs).end);
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.peek().kind == TokenKind::LParen {
                self.bump();
                let mut args = Vec::new();
                if self.peek().kind != TokenKind::RParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.peek().kind == TokenKind::Comma {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect_punct(TokenKind::RParen)?.span.end;
                let span = Span::new(expr_span(&expr).start, end);
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let t = self.bump();
        match t.kind {
            TokenKind::Ident(name) => Ok(Expr::Ident {
                name,
                span: t.span,
            }),
            TokenKind::Int(value) => Ok(Expr::Int {
                value,
                span: t.span,
            }),
            TokenKind::Float(text) => Ok(Expr::Float {
                text,
                span: t.span,
            }),
            TokenKind::Str(value) => Ok(Expr::Str {
                value,
                span: t.span,
            }),
            TokenKind::LParen => {
                let inner = self.parse_expr()?;
                let end = self.expect_punct(TokenKind::RParen)?.span.end;
                Ok(Expr::Group {
                    inner: Box::new(inner),
                    span: Span::new(t.span.start, end),
                })
            }
            _ => Err(self.diag(t.span, "E0016", &format!("expected expression, found {:?}", t.kind))),
        }
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Ident { span, .. }
        | Expr::Int { span, .. }
        | Expr::Float { span, .. }
        | Expr::Str { span, .. }
        | Expr::Call { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Group { span, .. } => *span,
    }
}

/// Parse a source file into an AST.
pub fn parse_file(path: &str, text: String) -> Result<(SourceFile, Program), Diagnostic> {
    let file = SourceFile {
        path: path.to_string(),
        text: text.clone(),
    };
    let tokens = Lexer::new(path, &text).tokenize()?;
    let mut parser = Parser::new(path, tokens);
    let program = parser.parse_program()?;
    Ok((file, program))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_hello() {
        let src = r#"fn main() { print("hello, newproj") }"#;
        let toks = Lexer::new("t.np", src).tokenize().unwrap();
        assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::Fn)));
        assert!(toks.iter().any(|t| matches!(t.kind, TokenKind::Str(_))));
    }

    #[test]
    fn parses_hello() {
        let src = r#"
# 01 — hello
fn main() {
    print("hello, newproj")
}
"#;
        let (_f, prog) = parse_file("01_hello.np", src.into()).unwrap();
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            Item::Fn(f) => {
                assert_eq!(f.name, "main");
                assert_eq!(f.body.stmts.len(), 1);
            }
        }
    }

    #[test]
    fn parses_locals() {
        let src = r#"
fn main() {
    let name = "world"
    let n: i32 = 42
    print(name)
    print(n)
}
"#;
        let (_f, prog) = parse_file("02_locals.np", src.into()).unwrap();
        match &prog.items[0] {
            Item::Fn(f) => assert!(f.body.stmts.len() >= 3),
        }
    }

    #[test]
    fn empty_program_ok() {
        assert!(parse_file("x.np", String::new()).is_ok());
    }

    #[test]
    fn rejects_bad_token() {
        let err = parse_file("x.np", "fn main() { @ }".into()).unwrap_err();
        assert_eq!(err.code, "E0002");
    }
}
