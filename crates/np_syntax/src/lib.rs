//! Lexer and parser for `.np` source. Spec = `examples/`.
//!
//! Compiler layout:
//! - `crates/np_syntax` — this crate (tokens + AST + parse)
//! - `crates/np_hir` — early checks / typed IR (stub)
//! - `crates/npc` — CLI (`npc check`)

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
    As,
    Is,
    Const,
    Ident(String),
    Int(i64),
    Float(String),
    Str(String),
    CStr(String),
    Char(char),
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
    Arrow,
    FatArrow,
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

    fn read_string_body(&mut self, start: usize) -> Result<String, Diagnostic> {
        let mut s = String::new();
        loop {
            match self.bump() {
                Some(b'"') => return Ok(s),
                Some(b'\\') => match self.bump() {
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(b'0') => s.push('\0'),
                    Some(b'"') => s.push('"'),
                    Some(b'\\') => s.push('\\'),
                    Some(o) => s.push(o as char),
                    None => {
                        return Err(self.err(start, self.pos, "E0003", "unterminated string escape"))
                    }
                },
                Some(ch) => s.push(ch as char),
                None => return Err(self.err(start, self.pos, "E0003", "unterminated string")),
            }
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
            b'"' => TokenKind::Str(self.read_string_body(start)?),
            b'\'' => {
                let ch = match self.bump() {
                    Some(b'\\') => match self.bump() {
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        Some(b'0') => '\0',
                        Some(b'\'') => '\'',
                        Some(b'\\') => '\\',
                        Some(o) => o as char,
                        None => {
                            return Err(self.err(start, self.pos, "E0005", "unterminated char"))
                        }
                    },
                    Some(b'\'') => {
                        return Err(self.err(start, self.pos, "E0005", "empty char literal"))
                    }
                    Some(o) => o as char,
                    None => return Err(self.err(start, self.pos, "E0005", "unterminated char")),
                };
                if self.bump() != Some(b'\'') {
                    return Err(self.err(start, self.pos, "E0005", "unterminated char literal"));
                }
                TokenKind::Char(ch)
            }
            b'0'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump();
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
                    let text = &self.src[start..self.pos];
                    let n: i64 = text.parse().map_err(|_| {
                        self.err(start, self.pos, "E0004", &format!("invalid integer `{text}`"))
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
                // c"..." C string
                if text == "c" && self.peek() == Some(b'"') {
                    self.bump();
                    TokenKind::CStr(self.read_string_body(start)?)
                } else {
                    keyword_or_ident(text)
                }
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

fn keyword_or_ident(text: &str) -> TokenKind {
    match text {
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
        "as" => TokenKind::As,
        "is" => TokenKind::Is,
        "const" => TokenKind::Const,
        _ => TokenKind::Ident(text.to_string()),
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
    Type(TypeItem),
    Extern(ExternBlock),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnItem {
    pub receiver: Option<String>, // Point in `fn Point.dist`
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
    pub ty: Option<TypeRef>, // None for bare `self`
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Named {
        name: String,
        args: Vec<TypeRef>,
        span: Span,
    },
    Ptr {
        is_const: bool,
        inner: Box<TypeRef>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeItem {
    pub name: String,
    pub name_span: Span,
    pub kind: TypeBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeBody {
    Struct(Vec<Field>),
    Adt(Vec<Variant>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternBlock {
    pub abi: String,
    pub items: Vec<ExternFn>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternFn {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeRef>,
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
    Ident {
        name: String,
        span: Span,
    },
    Int {
        value: i64,
        span: Span,
    },
    Float {
        text: String,
        span: Span,
    },
    Str {
        value: String,
        span: Span,
    },
    CStr {
        value: String,
        span: Span,
    },
    Char {
        value: char,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Field {
        base: Box<Expr>,
        name: String,
        span: Span,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Cast {
        expr: Box<Expr>,
        ty: TypeRef,
        span: Span,
    },
    Is {
        expr: Box<Expr>,
        pat: Pat,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    Task {
        body: Block,
        span: Span,
    },
    Await {
        inner: Box<Expr>,
        span: Span,
    },
    Unsafe {
        body: Block,
        span: Span,
    },
    Group {
        inner: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pat: Pat,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pat {
    Ident {
        name: String,
        span: Span,
    },
    Call {
        name: String,
        args: Vec<Pat>,
        span: Span,
    },
    Struct {
        name: String,
        fields: Vec<(String, Pat)>,
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
            .unwrap_or_else(|| self.tokens.last().expect("eof"))
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
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
        while !self.at(&TokenKind::Eof) {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        match self.peek_kind() {
            TokenKind::Fn => Ok(Item::Fn(self.parse_fn()?)),
            TokenKind::Type => Ok(Item::Type(self.parse_type_item()?)),
            TokenKind::Extern => Ok(Item::Extern(self.parse_extern()?)),
            _ => {
                let t = self.peek().clone();
                Err(self.diag(t.span, "E0011", "expected `fn`, `type`, or `extern` item"))
            }
        }
    }

    fn parse_fn(&mut self) -> Result<FnItem, Diagnostic> {
        let start = self.expect_punct(TokenKind::Fn)?.span.start;
        let first = self.bump();
        let (receiver, name, name_span) = match first.kind {
            TokenKind::Ident(s) => {
                if self.at(&TokenKind::Dot) {
                    self.bump();
                    let second = self.bump();
                    match second.kind {
                        TokenKind::Ident(n) => (Some(s), n, second.span),
                        _ => {
                            return Err(self.diag(second.span, "E0012", "expected method name"));
                        }
                    }
                } else {
                    (None, s, first.span)
                }
            }
            _ => return Err(self.diag(first.span, "E0012", "expected function name")),
        };
        self.expect_punct(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if self.at(&TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_punct(TokenKind::RParen)?;
        let ret = if self.at(&TokenKind::Arrow) {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = Span::new(start, body.span.end);
        Ok(FnItem {
            receiver,
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
        if name == "self" && !self.at(&TokenKind::Colon) {
            return Ok(Param {
                name,
                ty: None,
                span: name_tok.span,
            });
        }
        self.expect_punct(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        let span = Span::new(start, type_span(&ty).end);
        Ok(Param {
            name,
            ty: Some(ty),
            span,
        })
    }

    fn parse_type(&mut self) -> Result<TypeRef, Diagnostic> {
        if self.at(&TokenKind::Star) {
            let start = self.bump().span.start;
            let is_const = if self.at(&TokenKind::Const) {
                self.bump();
                true
            } else {
                false
            };
            let inner = self.parse_type()?;
            let span = Span::new(start, type_span(&inner).end);
            return Ok(TypeRef::Ptr {
                is_const,
                inner: Box::new(inner),
                span,
            });
        }
        let t = self.bump();
        let name = match t.kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.diag(t.span, "E0014", "expected type name")),
        };
        let mut args = Vec::new();
        let mut end = t.span.end;
        if self.at(&TokenKind::Lt) {
            self.bump();
            if !self.at(&TokenKind::Gt) {
                loop {
                    args.push(self.parse_type()?);
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            end = self.expect_punct(TokenKind::Gt)?.span.end;
        }
        Ok(TypeRef::Named {
            name,
            args,
            span: Span::new(t.span.start, end),
        })
    }

    fn parse_type_item(&mut self) -> Result<TypeItem, Diagnostic> {
        let start = self.expect_punct(TokenKind::Type)?.span.start;
        let name_tok = self.bump();
        let (name, name_span) = match name_tok.kind {
            TokenKind::Ident(n) => (n, name_tok.span),
            _ => return Err(self.diag(name_tok.span, "E0017", "expected type name")),
        };
        self.expect_punct(TokenKind::LBrace)?;
        // Peek first member: `Name {` => ADT variant, `name:` => struct field
        let kind = if matches!(self.peek_kind(), TokenKind::Ident(_)) {
            // Lookahead: Ident then `{` => variant; Ident then `:` => field
            let save = self.idx;
            let _ = self.bump();
            let is_variant = self.at(&TokenKind::LBrace);
            self.idx = save;
            if is_variant {
                let mut variants = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    variants.push(self.parse_variant()?);
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                    }
                }
                TypeBody::Adt(variants)
            } else {
                let mut fields = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    fields.push(self.parse_field()?);
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                    }
                }
                TypeBody::Struct(fields)
            }
        } else {
            TypeBody::Struct(Vec::new())
        };
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(TypeItem {
            name,
            name_span,
            kind,
            span: Span::new(start, end),
        })
    }

    fn parse_variant(&mut self) -> Result<Variant, Diagnostic> {
        let name_tok = self.bump();
        let (name, start) = match name_tok.kind {
            TokenKind::Ident(n) => (n, name_tok.span.start),
            _ => return Err(self.diag(name_tok.span, "E0018", "expected variant name")),
        };
        self.expect_punct(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            fields.push(self.parse_field()?);
            if self.at(&TokenKind::Comma) {
                self.bump();
            }
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(Variant {
            name,
            fields,
            span: Span::new(start, end),
        })
    }

    fn parse_field(&mut self) -> Result<Field, Diagnostic> {
        let name_tok = self.bump();
        let (name, start) = match name_tok.kind {
            TokenKind::Ident(n) => (n, name_tok.span.start),
            _ => return Err(self.diag(name_tok.span, "E0019", "expected field name")),
        };
        self.expect_punct(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        Ok(Field {
            name,
            span: Span::new(start, type_span(&ty).end),
            ty,
        })
    }

    fn parse_extern(&mut self) -> Result<ExternBlock, Diagnostic> {
        let start = self.expect_punct(TokenKind::Extern)?.span.start;
        let abi_tok = self.bump();
        let abi = match abi_tok.kind {
            TokenKind::Str(s) => s,
            _ => return Err(self.diag(abi_tok.span, "E0020", "expected ABI string, e.g. \"C\"")),
        };
        self.expect_punct(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            items.push(self.parse_extern_fn()?);
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(ExternBlock {
            abi,
            items,
            span: Span::new(start, end),
        })
    }

    fn parse_extern_fn(&mut self) -> Result<ExternFn, Diagnostic> {
        let start = self.expect_punct(TokenKind::Fn)?.span.start;
        let name_tok = self.bump();
        let name = match name_tok.kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.diag(name_tok.span, "E0012", "expected function name")),
        };
        self.expect_punct(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if self.at(&TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect_punct(TokenKind::RParen)?;
        let ret = if self.at(&TokenKind::Arrow) {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        let end = ret
            .as_ref()
            .map(type_span)
            .map(|s| s.end)
            .unwrap_or_else(|| self.tokens[self.idx.saturating_sub(1)].span.end);
        if self.at(&TokenKind::Semi) {
            self.bump();
        }
        Ok(ExternFn {
            name,
            params,
            ret,
            span: Span::new(start, end),
        })
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect_punct(TokenKind::LBrace)?.span.start;
        let mut stmts = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(Block {
            stmts,
            span: Span::new(start, end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Return => {
                let start = self.bump().span.start;
                let value = if matches!(
                    self.peek_kind(),
                    TokenKind::RBrace | TokenKind::Semi | TokenKind::Eof | TokenKind::Comma
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
                if self.at(&TokenKind::Semi) {
                    self.bump();
                }
                Ok(Stmt::Return {
                    value,
                    span: Span::new(start, end),
                })
            }
            _ => {
                let e = self.parse_expr()?;
                if self.at(&TokenKind::Semi) {
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
        let ty = if self.at(&TokenKind::Colon) {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_punct(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        let end = expr_span(&init).end;
        if self.at(&TokenKind::Semi) {
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
        while self.at(&TokenKind::OrOr) {
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
        let mut lhs = self.parse_is()?;
        while self.at(&TokenKind::AndAnd) {
            self.bump();
            let rhs = self.parse_is()?;
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

    fn parse_is(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_cmp()?;
        if self.at(&TokenKind::Is) {
            self.bump();
            let pat = self.parse_pat()?;
            let span = Span::new(expr_span(&lhs).start, pat_span(&pat).end);
            lhs = Expr::Is {
                expr: Box::new(lhs),
                pat,
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek_kind() {
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
            let op = match self.peek_kind() {
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
        let mut lhs = self.parse_as()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_as()?;
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

    fn parse_as(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        while self.at(&TokenKind::As) {
            self.bump();
            let ty = self.parse_type()?;
            let span = Span::new(expr_span(&expr).start, type_span(&ty).end);
            expr = Expr::Cast {
                expr: Box::new(expr),
                ty,
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.at(&TokenKind::Await) {
            let start = self.bump().span.start;
            let inner = self.parse_unary()?;
            let span = Span::new(start, expr_span(&inner).end);
            return Ok(Expr::Await {
                inner: Box::new(inner),
                span,
            });
        }
        if self.at(&TokenKind::Minus) {
            let start = self.bump().span.start;
            let inner = self.parse_unary()?;
            let span = Span::new(start, expr_span(&inner).end);
            return Ok(Expr::Binary {
                op: BinOp::Sub,
                lhs: Box::new(Expr::Int {
                    value: 0,
                    span: Span::new(start, start),
                }),
                rhs: Box::new(inner),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.at(&TokenKind::LParen) {
                self.bump();
                let mut args = Vec::new();
                if !self.at(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.at(&TokenKind::Comma) {
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
            } else if self.at(&TokenKind::LBracket) {
                self.bump();
                let index = self.parse_expr()?;
                let end = self.expect_punct(TokenKind::RBracket)?.span.end;
                let span = Span::new(expr_span(&expr).start, end);
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.at(&TokenKind::Dot) {
                self.bump();
                let name_tok = self.bump();
                let name = match name_tok.kind {
                    TokenKind::Ident(n) => n,
                    _ => return Err(self.diag(name_tok.span, "E0021", "expected field name")),
                };
                let span = Span::new(expr_span(&expr).start, name_tok.span.end);
                expr = Expr::Field {
                    base: Box::new(expr),
                    name,
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek_kind().clone() {
            TokenKind::If => self.parse_if(),
            TokenKind::Match => self.parse_match(),
            TokenKind::Task => {
                let start = self.bump().span.start;
                let body = self.parse_block()?;
                Ok(Expr::Task {
                    span: Span::new(start, body.span.end),
                    body,
                })
            }
            TokenKind::Unsafe => {
                let start = self.bump().span.start;
                let body = self.parse_block()?;
                Ok(Expr::Unsafe {
                    span: Span::new(start, body.span.end),
                    body,
                })
            }
            TokenKind::Ident(name) => {
                let t = self.bump();
                // Struct literal: Name { field: expr }
                if self.at(&TokenKind::LBrace) {
                    // Ambiguity with blocks after if/match — only treat as struct lit
                    // when next is Ident + Colon (field) or RBrace.
                    let save = self.idx;
                    self.bump(); // {
                    let is_struct = self.at(&TokenKind::RBrace)
                        || (matches!(self.peek_kind(), TokenKind::Ident(_)) && {
                            let save2 = self.idx;
                            self.bump();
                            let ok = self.at(&TokenKind::Colon);
                            self.idx = save2;
                            ok
                        });
                    self.idx = save;
                    if is_struct {
                        return self.parse_struct_lit(name, t.span.start);
                    }
                }
                Ok(Expr::Ident {
                    name,
                    span: t.span,
                })
            }
            TokenKind::Int(value) => {
                let t = self.bump();
                Ok(Expr::Int {
                    value,
                    span: t.span,
                })
            }
            TokenKind::Float(text) => {
                let t = self.bump();
                Ok(Expr::Float {
                    text,
                    span: t.span,
                })
            }
            TokenKind::Str(value) => {
                let t = self.bump();
                Ok(Expr::Str {
                    value,
                    span: t.span,
                })
            }
            TokenKind::CStr(value) => {
                let t = self.bump();
                Ok(Expr::CStr {
                    value,
                    span: t.span,
                })
            }
            TokenKind::Char(value) => {
                let t = self.bump();
                Ok(Expr::Char {
                    value,
                    span: t.span,
                })
            }
            TokenKind::LParen => {
                let start = self.bump().span.start;
                let inner = self.parse_expr()?;
                let end = self.expect_punct(TokenKind::RParen)?.span.end;
                Ok(Expr::Group {
                    inner: Box::new(inner),
                    span: Span::new(start, end),
                })
            }
            other => {
                let t = self.bump();
                Err(self.diag(
                    t.span,
                    "E0016",
                    &format!("expected expression, found {:?}", other),
                ))
            }
        }
    }

    fn parse_struct_lit(&mut self, name: String, start: usize) -> Result<Expr, Diagnostic> {
        self.expect_punct(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let fname_tok = self.bump();
            let fname = match fname_tok.kind {
                TokenKind::Ident(n) => n,
                _ => return Err(self.diag(fname_tok.span, "E0022", "expected field name")),
            };
            self.expect_punct(TokenKind::Colon)?;
            let val = self.parse_expr()?;
            fields.push((fname, val));
            if self.at(&TokenKind::Comma) {
                self.bump();
            }
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(Expr::StructLit {
            name,
            fields,
            span: Span::new(start, end),
        })
    }

    fn parse_if(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect_punct(TokenKind::If)?.span.start;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.at(&TokenKind::Else) {
            self.bump();
            Some(self.parse_block()?)
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map(|b| b.span.end)
            .unwrap_or(then_block.span.end);
        Ok(Expr::If {
            cond: Box::new(cond),
            then_block,
            else_block,
            span: Span::new(start, end),
        })
    }

    fn parse_match(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect_punct(TokenKind::Match)?.span.start;
        let scrutinee = self.parse_expr()?;
        self.expect_punct(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let arm_start = self.peek().span.start;
            let pat = self.parse_pat()?;
            self.expect_punct(TokenKind::FatArrow)?;
            let body = self.parse_expr()?;
            let span = Span::new(arm_start, expr_span(&body).end);
            arms.push(MatchArm { pat, body, span });
            if self.at(&TokenKind::Comma) {
                self.bump();
            }
        }
        let end = self.expect_punct(TokenKind::RBrace)?.span.end;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span: Span::new(start, end),
        })
    }

    fn parse_pat(&mut self) -> Result<Pat, Diagnostic> {
        let t = self.bump();
        let name = match t.kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.diag(t.span, "E0023", "expected pattern")),
        };
        if self.at(&TokenKind::LParen) {
            self.bump();
            let mut args = Vec::new();
            if !self.at(&TokenKind::RParen) {
                loop {
                    args.push(self.parse_pat()?);
                    if self.at(&TokenKind::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            let end = self.expect_punct(TokenKind::RParen)?.span.end;
            return Ok(Pat::Call {
                name,
                args,
                span: Span::new(t.span.start, end),
            });
        }
        // Struct pattern only when `{` starts fields (`r`, `r: pat`), not an `if` body
        // after `x is None { ... }`.
        if self.at(&TokenKind::LBrace) && self.lookahead_struct_pat_fields() {
            self.bump();
            let mut fields = Vec::new();
            while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                let ftok = self.bump();
                let fname = match ftok.kind {
                    TokenKind::Ident(n) => n,
                    _ => return Err(self.diag(ftok.span, "E0022", "expected field name")),
                };
                let pat = if self.at(&TokenKind::Colon) {
                    self.bump();
                    self.parse_pat()?
                } else {
                    Pat::Ident {
                        name: fname.clone(),
                        span: ftok.span,
                    }
                };
                fields.push((fname, pat));
                if self.at(&TokenKind::Comma) {
                    self.bump();
                }
            }
            let end = self.expect_punct(TokenKind::RBrace)?.span.end;
            return Ok(Pat::Struct {
                name,
                fields,
                span: Span::new(t.span.start, end),
            });
        }
        Ok(Pat::Ident {
            name,
            span: t.span,
        })
    }

    fn lookahead_struct_pat_fields(&mut self) -> bool {
        let save = self.idx;
        self.bump(); // {
        let ok = match self.peek_kind() {
            TokenKind::RBrace => true,
            TokenKind::Ident(_) => {
                self.bump();
                matches!(
                    self.peek_kind(),
                    TokenKind::Colon | TokenKind::Comma | TokenKind::RBrace
                )
            }
            _ => false,
        };
        self.idx = save;
        ok
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Ident { span, .. }
        | Expr::Int { span, .. }
        | Expr::Float { span, .. }
        | Expr::Str { span, .. }
        | Expr::CStr { span, .. }
        | Expr::Char { span, .. }
        | Expr::Call { span, .. }
        | Expr::Field { span, .. }
        | Expr::Index { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Cast { span, .. }
        | Expr::Is { span, .. }
        | Expr::If { span, .. }
        | Expr::Match { span, .. }
        | Expr::StructLit { span, .. }
        | Expr::Task { span, .. }
        | Expr::Await { span, .. }
        | Expr::Unsafe { span, .. }
        | Expr::Group { span, .. } => *span,
    }
}

fn type_span(t: &TypeRef) -> Span {
    match t {
        TypeRef::Named { span, .. } | TypeRef::Ptr { span, .. } => *span,
    }
}

fn pat_span(p: &Pat) -> Span {
    match p {
        Pat::Ident { span, .. } | Pat::Call { span, .. } | Pat::Struct { span, .. } => *span,
    }
}

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
    fn parses_hello() {
        let src = r#"fn main() { print("hello, newproj") }"#;
        let (_f, prog) = parse_file("t.np", src.into()).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parses_option_match() {
        let src = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/03_option_match.np"),
        )
        .unwrap();
        parse_file("03.np", src).unwrap();
    }

    #[test]
    fn parses_adt() {
        let src = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/05_adt.np"),
        )
        .unwrap();
        parse_file("05.np", src).unwrap();
    }

    #[test]
    fn parses_c_abi() {
        let src = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/09_c_abi.np"),
        )
        .unwrap();
        parse_file("09.np", src).unwrap();
    }

    #[test]
    fn rejects_bad_token() {
        let err = parse_file("x.np", "fn main() { @ }".into()).unwrap_err();
        assert_eq!(err.code, "E0002");
    }
}
