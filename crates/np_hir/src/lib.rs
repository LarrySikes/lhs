//! Name resolution + type checking for `.lhs` programs.

use std::collections::HashMap;

use np_syntax::{
    BinOp, Block, Expr, FnItem, Item, Pat, Program, SourceFile, Stmt, TypeBody, TypeItem, TypeRef,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub code: String,
    pub message: String,
    pub help: Option<String>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}: {} {}",
            self.file, self.start, self.end, self.code, self.message
        )
    }
}

impl From<np_syntax::Diagnostic> for Diagnostic {
    fn from(d: np_syntax::Diagnostic) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Unit,
    Bool,
    I32,
    I64,
    U8,
    F64,
    Str,
    Char,
    Named(String),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    Ptr {
        is_const: bool,
        inner: Box<Ty>,
    },
    /// Inferred / not yet known
    Unknown,
    /// Type error placeholder
    Error,
}

impl Ty {
    fn display(&self) -> String {
        match self {
            Ty::Unit => "()".into(),
            Ty::Bool => "bool".into(),
            Ty::I32 => "i32".into(),
            Ty::I64 => "i64".into(),
            Ty::U8 => "u8".into(),
            Ty::F64 => "f64".into(),
            Ty::Str => "str".into(),
            Ty::Char => "char".into(),
            Ty::Named(n) => n.clone(),
            Ty::Option(t) => format!("Option<{}>", t.display()),
            Ty::Result(ok, err) => format!("Result<{}, {}>", ok.display(), err.display()),
            Ty::Ptr { is_const, inner } => {
                if *is_const {
                    format!("*const {}", inner.display())
                } else {
                    format!("*{}", inner.display())
                }
            }
            Ty::Unknown => "_".into(),
            Ty::Error => "<error>".into(),
        }
    }

    fn same(&self, other: &Ty) -> bool {
        match (self, other) {
            (Ty::Error, _) | (_, Ty::Error) | (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            (Ty::Option(a), Ty::Option(b)) => a.same(b),
            (Ty::Result(a1, a2), Ty::Result(b1, b2)) => a1.same(b1) && a2.same(b2),
            (Ty::Ptr { is_const: c1, inner: i1 }, Ty::Ptr { is_const: c2, inner: i2 }) => {
                c1 == c2 && i1.same(i2)
            }
            (a, b) => a == b,
        }
    }
}

#[derive(Debug)]
pub struct Module {
    pub file: SourceFile,
    pub program: Program,
}

struct Checker<'a> {
    file: &'a str,
    types: HashMap<String, &'a TypeItem>,
    // fn key "name" or "Type.name" -> (params, ret)
    fns: HashMap<String, (Vec<Ty>, Ty)>,
    diags: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn diag(&mut self, start: usize, end: usize, code: &str, message: impl Into<String>, help: Option<&str>) {
        self.diags.push(Diagnostic {
            file: self.file.to_string(),
            start,
            end,
            code: code.into(),
            message: message.into(),
            help: help.map(|s| s.to_string()),
        });
    }

    fn resolve_type(&mut self, t: &TypeRef) -> Ty {
        match t {
            TypeRef::Named { name, args, span } => match name.as_str() {
                "i32" => Ty::I32,
                "i64" => Ty::I64,
                "u8" => Ty::U8,
                "f64" => Ty::F64,
                "bool" => Ty::Bool,
                "str" => Ty::Str,
                "char" => Ty::Char,
                "()" => Ty::Unit,
                "Option" => {
                    if args.len() != 1 {
                        self.diag(span.start, span.end, "E0100", "Option needs 1 type arg", None);
                        return Ty::Error;
                    }
                    Ty::Option(Box::new(self.resolve_type(&args[0])))
                }
                "Result" => {
                    if args.len() != 2 {
                        self.diag(span.start, span.end, "E0101", "Result needs 2 type args", None);
                        return Ty::Error;
                    }
                    Ty::Result(
                        Box::new(self.resolve_type(&args[0])),
                        Box::new(self.resolve_type(&args[1])),
                    )
                }
                other => {
                    if args.is_empty() {
                        if self.types.contains_key(other)
                            || other.chars().next().is_some_and(|c| c.is_uppercase())
                        {
                            Ty::Named(other.to_string())
                        } else {
                            self.diag(
                                span.start,
                                span.end,
                                "E0102",
                                format!("unknown type `{other}`"),
                                None,
                            );
                            Ty::Error
                        }
                    } else {
                        self.diag(
                            span.start,
                            span.end,
                            "E0103",
                            format!("type `{other}` does not take type arguments"),
                            None,
                        );
                        Ty::Error
                    }
                }
            },
            TypeRef::Ptr {
                is_const,
                inner,
                ..
            } => Ty::Ptr {
                is_const: *is_const,
                inner: Box::new(self.resolve_type(inner)),
            },
        }
    }

    fn collect(&mut self, program: &'a Program) {
        for item in &program.items {
            match item {
                Item::Type(t) => {
                    self.types.insert(t.name.clone(), t);
                }
                Item::Fn(f) => {
                    let key = match &f.receiver {
                        Some(r) => format!("{r}.{}", f.name),
                        None => f.name.clone(),
                    };
                    let mut params = Vec::new();
                    for p in &f.params {
                        if p.name == "self" && p.ty.is_none() {
                            if let Some(r) = &f.receiver {
                                params.push(Ty::Named(r.clone()));
                            } else {
                                params.push(Ty::Error);
                            }
                        } else if let Some(ty) = &p.ty {
                            params.push(self.resolve_type(ty));
                        } else {
                            params.push(Ty::Unknown);
                        }
                    }
                    let ret = f
                        .ret
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(Ty::Unit);
                    self.fns.insert(key, (params, ret));
                }
                Item::Extern(ext) => {
                    for ef in &ext.items {
                        let mut params = Vec::new();
                        for p in &ef.params {
                            if let Some(ty) = &p.ty {
                                params.push(self.resolve_type(ty));
                            } else {
                                params.push(Ty::Unknown);
                            }
                        }
                        let ret = ef
                            .ret
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit);
                        self.fns.insert(ef.name.clone(), (params, ret));
                    }
                }
            }
        }
        // builtins
        self.fns.insert("print".into(), (vec![Ty::Unknown], Ty::Unit));
        self.fns.insert(
            "read_file".into(),
            (
                vec![Ty::Str],
                Ty::Result(Box::new(Ty::Str), Box::new(Ty::Str)),
            ),
        );
        self.fns.insert(
            "write_file".into(),
            (
                vec![Ty::Str, Ty::Str],
                Ty::Result(Box::new(Ty::Unit), Box::new(Ty::Str)),
            ),
        );
        self.fns.insert("abs".into(), (vec![Ty::Unknown], Ty::Unknown));
        self.fns
            .insert("min".into(), (vec![Ty::Unknown, Ty::Unknown], Ty::Unknown));
        self.fns
            .insert("max".into(), (vec![Ty::Unknown, Ty::Unknown], Ty::Unknown));
        self.fns
            .insert("assert".into(), (vec![Ty::Bool], Ty::Unit));
    }

    fn check_program(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Fn(f) = item {
                self.check_fn(f);
            }
        }
    }

    fn check_fn(&mut self, f: &FnItem) {
        let mut env = HashMap::new();
        for p in &f.params {
            if p.name == "self" && p.ty.is_none() {
                if let Some(r) = &f.receiver {
                    env.insert(p.name.clone(), Ty::Named(r.clone()));
                }
            } else if let Some(ty) = &p.ty {
                env.insert(p.name.clone(), self.resolve_type(ty));
            }
        }
        let ret = f
            .ret
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Ty::Unit);
        let body_ty = self.check_block(&mut env, &f.body, Some(&ret));
        if !body_ty.same(&ret) && body_ty != Ty::Unit && ret != Ty::Unit {
            // allow block ending with unit when explicit returns exist
            if !matches!(ret, Ty::Unit) && !body_ty.same(&ret) && body_ty != Ty::Error {
                let span = f.body.span;
                self.diag(
                    span.start,
                    span.end,
                    "E0110",
                    format!(
                        "function `{}` returns {}, but body has type {}",
                        f.name,
                        ret.display(),
                        body_ty.display()
                    ),
                    None,
                );
            }
        }
    }

    fn check_block(
        &mut self,
        env: &mut HashMap<String, Ty>,
        block: &Block,
        expected_ret: Option<&Ty>,
    ) -> Ty {
        let mut last = Ty::Unit;
        for stmt in &block.stmts {
            last = self.check_stmt(env, stmt, expected_ret);
        }
        last
    }

    fn check_stmt(
        &mut self,
        env: &mut HashMap<String, Ty>,
        stmt: &Stmt,
        expected_ret: Option<&Ty>,
    ) -> Ty {
        match stmt {
            Stmt::Let { name, ty, init, span } => {
                let init_ty = self.check_expr(env, init, None);
                let final_ty = if let Some(ann) = ty {
                    let want = self.resolve_type(ann);
                    if !want.same(&init_ty) {
                        let s = np_syntax::expr_span_pub(init);
                        self.diag(
                            s.start,
                            s.end,
                            "E0001",
                            format!(
                                "type mismatch: expected {}, found {}",
                                want.display(),
                                init_ty.display()
                            ),
                            Some("fix the annotation or the initializer"),
                        );
                    }
                    want
                } else {
                    init_ty
                };
                env.insert(name.clone(), final_ty);
                let _ = span;
                Ty::Unit
            }
            Stmt::Expr(e) => self.check_expr(env, e, None),
            Stmt::Return { value, span } => {
                let got = match value {
                    Some(e) => self.check_expr(env, e, expected_ret),
                    None => Ty::Unit,
                };
                if let Some(want) = expected_ret {
                    if !want.same(&got) {
                        self.diag(
                            span.start,
                            span.end,
                            "E0111",
                            format!(
                                "return type mismatch: expected {}, found {}",
                                want.display(),
                                got.display()
                            ),
                            None,
                        );
                    }
                }
                Ty::Unit
            }
        }
    }

    fn check_expr(
        &mut self,
        env: &mut HashMap<String, Ty>,
        expr: &Expr,
        hint: Option<&Ty>,
    ) -> Ty {
        match expr {
            Expr::Ident { name, span } => {
                if let Some(t) = env.get(name) {
                    return t.clone();
                }
                if name == "None" {
                    if let Some(Ty::Option(inner)) = hint {
                        return Ty::Option(inner.clone());
                    }
                    return Ty::Option(Box::new(Ty::Unknown));
                }
                self.diag(
                    span.start,
                    span.end,
                    "E0120",
                    format!("undefined variable `{name}`"),
                    None,
                );
                Ty::Error
            }
            Expr::Int { .. } => Ty::I32,
            Expr::Float { .. } => Ty::F64,
            Expr::Str { .. } => Ty::Str,
            Expr::CStr { .. } => Ty::Ptr {
                is_const: true,
                inner: Box::new(Ty::U8),
            },
            Expr::Char { .. } => Ty::Char,
            Expr::Group { inner, .. } => self.check_expr(env, inner, hint),
            Expr::Binary { op, lhs, rhs, span } => {
                let lt = self.check_expr(env, lhs, None);
                let rt = self.check_expr(env, rhs, None);
                self.check_binary(*op, &lt, &rt, span.start, span.end)
            }
            Expr::Cast { expr, ty, .. } => {
                let _ = self.check_expr(env, expr, None);
                self.resolve_type(ty)
            }
            Expr::Is { expr, .. } => {
                let _ = self.check_expr(env, expr, None);
                Ty::Bool
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                span,
            } => {
                let ct = self.check_expr(env, cond, Some(&Ty::Bool));
                if !ct.same(&Ty::Bool) {
                    self.diag(
                        span.start,
                        span.end,
                        "E0121",
                        format!("if condition must be bool, found {}", ct.display()),
                        None,
                    );
                }
                let t1 = self.check_block(env, then_block, None);
                if let Some(eb) = else_block {
                    let t2 = self.check_block(env, eb, None);
                    if t1.same(&t2) {
                        t1
                    } else if t1 == Ty::Unit || t2 == Ty::Unit {
                        Ty::Unit
                    } else {
                        t1
                    }
                } else {
                    Ty::Unit
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let st = self.check_expr(env, scrutinee, None);
                let mut result = Ty::Unknown;
                for arm in arms {
                    let mut local = env.clone();
                    self.bind_pat(&mut local, &arm.pat, &st);
                    let at = self.check_expr(&mut local, &arm.body, hint);
                    if matches!(result, Ty::Unknown) {
                        result = at;
                    }
                }
                result
            }
            Expr::StructLit { name, fields, span } => {
                if let Some(ti) = self.types.get(name).copied() {
                    match &ti.kind {
                        TypeBody::Struct(fs) => {
                            for (fname, fexpr) in fields {
                                if let Some(field) = fs.iter().find(|f| f.name == *fname) {
                                    let want = self.resolve_type(&field.ty);
                                    let got = self.check_expr(env, fexpr, Some(&want));
                                    if !want.same(&got) {
                                        let s = np_syntax::expr_span_pub(fexpr);
                                        self.diag(
                                            s.start,
                                            s.end,
                                            "E0001",
                                            format!(
                                                "field `{fname}`: expected {}, found {}",
                                                want.display(),
                                                got.display()
                                            ),
                                            None,
                                        );
                                    }
                                } else {
                                    self.diag(
                                        span.start,
                                        span.end,
                                        "E0122",
                                        format!("unknown field `{fname}` on `{name}`"),
                                        None,
                                    );
                                }
                            }
                        }
                        TypeBody::Adt(variants) => {
                            // StructLit used for variant construction Circle { r: ... }
                            if let Some(v) = variants.iter().find(|v| v.name == *name) {
                                let _ = v;
                            }
                            for (_fname, fexpr) in fields {
                                let _ = self.check_expr(env, fexpr, None);
                            }
                        }
                    }
                } else {
                    // Variant constructor named like Circle — look through ADTs
                    let type_names: Vec<String> = self.types.keys().cloned().collect();
                    for tname in type_names {
                        if let Some(ti) = self.types.get(&tname) {
                            if let TypeBody::Adt(variants) = &ti.kind {
                                if variants.iter().any(|v| v.name == *name) {
                                    for (fname, fexpr) in fields {
                                        if let Some(field) = variants
                                            .iter()
                                            .find(|vv| vv.name == *name)
                                            .and_then(|vv| {
                                                vv.fields.iter().find(|f| f.name == *fname)
                                            })
                                        {
                                            let want = self.resolve_type(&field.ty);
                                            let got = self.check_expr(env, fexpr, Some(&want));
                                            if !want.same(&got) {
                                                let s = np_syntax::expr_span_pub(fexpr);
                                                self.diag(
                                                    s.start,
                                                    s.end,
                                                    "E0001",
                                                    format!(
                                                        "field `{fname}`: expected {}, found {}",
                                                        want.display(),
                                                        got.display()
                                                    ),
                                                    None,
                                                );
                                            }
                                        }
                                    }
                                    return Ty::Named(tname);
                                }
                            }
                        }
                    }
                    for (_n, e) in fields {
                        let _ = self.check_expr(env, e, None);
                    }
                }
                Ty::Named(name.clone())
            }
            Expr::Field { base, name, span } => {
                let bt = self.check_expr(env, base, None);
                if name == "len" && bt.same(&Ty::Str) {
                    return Ty::I32;
                }
                match &bt {
                    Ty::Named(tn) => {
                        if let Some(ti) = self.types.get(tn) {
                            if let TypeBody::Struct(fs) = &ti.kind {
                                if let Some(f) = fs.iter().find(|f| f.name == *name) {
                                    return self.resolve_type(&f.ty);
                                }
                            }
                        }
                        self.diag(
                            span.start,
                            span.end,
                            "E0123",
                            format!("no field `{name}` on type `{tn}`"),
                            None,
                        );
                        Ty::Error
                    }
                    _ => {
                        // method receiver typed later in Call
                        Ty::Unknown
                    }
                }
            }
            Expr::Index { base, index, span } => {
                let bt = self.check_expr(env, base, None);
                let it = self.check_expr(env, index, Some(&Ty::I32));
                if !bt.same(&Ty::Str) {
                    self.diag(
                        span.start,
                        span.end,
                        "E0124",
                        format!("index base must be str, found {}", bt.display()),
                        None,
                    );
                }
                if !it.same(&Ty::I32) {
                    self.diag(
                        span.start,
                        span.end,
                        "E0125",
                        format!("index must be i32, found {}", it.display()),
                        None,
                    );
                }
                Ty::Char
            }
            Expr::Call { callee, args, span } => self.check_call(env, callee, args, span.start, span.end, hint),
            Expr::Task { body, .. } => self.check_block(env, body, None),
            Expr::Await { inner, .. } => self.check_expr(env, inner, hint),
            Expr::Unsafe { body, .. } => self.check_block(env, body, None),
        }
    }

    fn check_call(
        &mut self,
        env: &mut HashMap<String, Ty>,
        callee: &Expr,
        args: &[Expr],
        start: usize,
        end: usize,
        hint: Option<&Ty>,
    ) -> Ty {
        // method call
        if let Expr::Field {
            base,
            name: method,
            ..
        } = callee
        {
            let recv_ty = self.check_expr(env, base, None);
            if method == "len" {
                return Ty::I32;
            }
            if method == "sqrt" {
                let _ = args;
                return Ty::F64;
            }
            if method == "unwrap" {
                match recv_ty {
                    Ty::Option(inner) | Ty::Result(inner, _) => return *inner,
                    _ => {
                        self.diag(start, end, "E0126", "unwrap requires Option or Result", None);
                        return Ty::Error;
                    }
                }
            }
            if let Ty::Named(tn) = &recv_ty {
                let key = format!("{tn}.{method}");
                if let Some((params, ret)) = self.fns.get(&key).cloned() {
                    // params include self
                    for (i, a) in args.iter().enumerate() {
                        let expect = params.get(i + 1).cloned();
                        let _ = self.check_expr(env, a, expect.as_ref());
                    }
                    return ret;
                }
            }
            for a in args {
                let _ = self.check_expr(env, a, None);
            }
            return Ty::Unknown;
        }

        if let Expr::Ident { name, .. } = callee {
            if name == "print" {
                for a in args {
                    let _ = self.check_expr(env, a, None);
                }
                return Ty::Unit;
            }
            if name == "read_file" || name == "write_file" || name == "abs" || name == "min"
                || name == "max" || name == "assert"
            {
                if let Some((params, ret)) = self.fns.get(name).cloned() {
                    for (i, a) in args.iter().enumerate() {
                        let expect = params.get(i).cloned();
                        let got = self.check_expr(env, a, expect.as_ref());
                        if name == "abs" || name == "min" || name == "max" {
                            // return type follows args
                            if i == 0 && name == "abs" {
                                return got;
                            }
                            if i == 0 && (name == "min" || name == "max") {
                                // fall through after all args
                            }
                        }
                    }
                    if name == "min" || name == "max" {
                        if let Some(a0) = args.first() {
                            return self.check_expr(env, a0, None);
                        }
                    }
                    return ret;
                }
            }
            if name == "Some" {
                let inner = if let Some(a) = args.first() {
                    self.check_expr(env, a, hint.and_then(|h| match h {
                        Ty::Option(i) => Some(i.as_ref()),
                        _ => None,
                    }).cloned().as_ref())
                } else {
                    Ty::Unknown
                };
                return Ty::Option(Box::new(inner));
            }
            if name == "Ok" {
                let ok = args
                    .first()
                    .map(|a| self.check_expr(env, a, None))
                    .unwrap_or(Ty::Unknown);
                let err_ty = match hint {
                    Some(Ty::Result(_, e)) => e.as_ref().clone(),
                    _ => Ty::Unknown,
                };
                return Ty::Result(Box::new(ok), Box::new(err_ty));
            }
            if name == "Err" {
                let err_t = args
                    .first()
                    .map(|a| self.check_expr(env, a, None))
                    .unwrap_or(Ty::Unknown);
                let ok_ty = match hint {
                    Some(Ty::Result(o, _)) => o.as_ref().clone(),
                    _ => Ty::Unknown,
                };
                return Ty::Result(Box::new(ok_ty), Box::new(err_t));
            }
            if let Some((params, ret)) = self.fns.get(name).cloned() {
                for (i, a) in args.iter().enumerate() {
                    let expect = params.get(i).cloned();
                    let got = self.check_expr(env, a, expect.as_ref());
                    if let Some(want) = expect {
                        if !want.same(&got) && want != Ty::Unknown {
                            let s = np_syntax::expr_span_pub(a);
                            self.diag(
                                s.start,
                                s.end,
                                "E0001",
                                format!(
                                    "argument {}: expected {}, found {}",
                                    i + 1,
                                    want.display(),
                                    got.display()
                                ),
                                None,
                            );
                        }
                    }
                }
                return ret;
            }
            self.diag(
                start,
                end,
                "E0127",
                format!("undefined function `{name}`"),
                None,
            );
            return Ty::Error;
        }

        Ty::Error
    }

    fn check_binary(&mut self, op: BinOp, lt: &Ty, rt: &Ty, start: usize, end: usize) -> Ty {
        use BinOp::*;
        match op {
            Add | Sub | Mul | Div => {
                if lt.same(&Ty::F64) || rt.same(&Ty::F64) {
                    Ty::F64
                } else if lt.same(&Ty::I32) && rt.same(&Ty::I32) {
                    Ty::I32
                } else if matches!(lt, Ty::Char) || matches!(rt, Ty::Char) {
                    // char arithmetic used in digit example
                    Ty::I32
                } else if lt.same(rt) {
                    lt.clone()
                } else {
                    self.diag(
                        start,
                        end,
                        "E0130",
                        format!(
                            "cannot apply arithmetic to {} and {}",
                            lt.display(),
                            rt.display()
                        ),
                        None,
                    );
                    Ty::Error
                }
            }
            Eq | Ne | Lt | Le | Gt | Ge => Ty::Bool,
            And | Or => {
                if !lt.same(&Ty::Bool) || !rt.same(&Ty::Bool) {
                    self.diag(start, end, "E0131", "logical ops need bool", None);
                }
                Ty::Bool
            }
        }
    }

    fn bind_pat(&mut self, env: &mut HashMap<String, Ty>, pat: &Pat, scrut: &Ty) {
        match pat {
            Pat::Ident { name, .. } => {
                if name == "None" || name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    // constructor nullary
                } else {
                    let ty = match scrut {
                        Ty::Option(inner) => *inner.clone(),
                        Ty::Result(ok, _) => *ok.clone(),
                        other => other.clone(),
                    };
                    env.insert(name.clone(), ty);
                }
            }
            Pat::Call { name, args, .. } => {
                let inner = match (name.as_str(), scrut) {
                    ("Some", Ty::Option(t)) => t.as_ref().clone(),
                    ("Ok", Ty::Result(t, _)) => t.as_ref().clone(),
                    ("Err", Ty::Result(_, e)) => e.as_ref().clone(),
                    _ => Ty::Unknown,
                };
                for a in args {
                    self.bind_pat(env, a, &inner);
                }
            }
            Pat::Struct { fields, .. } => {
                // bind field names from ADT — use Unknown or look up
                for (fname, fpat) in fields {
                    let fty = match scrut {
                        Ty::Named(tn) => {
                            if let Some(ti) = self.types.get(tn) {
                                match &ti.kind {
                                    TypeBody::Adt(variants) => variants
                                        .iter()
                                        .flat_map(|v| v.fields.iter())
                                        .find(|f| f.name == *fname)
                                        .map(|f| self.resolve_type(&f.ty))
                                        .unwrap_or(Ty::Unknown),
                                    TypeBody::Struct(fs) => fs
                                        .iter()
                                        .find(|f| f.name == *fname)
                                        .map(|f| self.resolve_type(&f.ty))
                                        .unwrap_or(Ty::Unknown),
                                }
                            } else {
                                Ty::Unknown
                            }
                        }
                        _ => Ty::F64, // Shape fields are f64 in examples
                    };
                    self.bind_pat(env, fpat, &fty);
                }
            }
        }
    }
}

pub fn check(path: &str, text: String) -> (Option<Module>, Vec<Diagnostic>) {
    match np_syntax::parse_file(path, text) {
        Ok((file, program)) => {
            let mut checker = Checker {
                file: path,
                types: HashMap::new(),
                fns: HashMap::new(),
                diags: Vec::new(),
            };
            checker.collect(&program);
            checker.check_program(&program);
            let diags = checker.diags;
            if diags.iter().any(|d| d.code.starts_with('E')) {
                // still return module for tooling that wants AST
            }
            (Some(Module { file, program }), diags)
        }
        Err(e) => (None, vec![Diagnostic::from(e)]),
    }
}
