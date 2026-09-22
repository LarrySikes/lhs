//! Pretty-printer for `.lhs` AST (`lhsc fmt`).

use crate::{
    BinOp, Block, Expr, FnItem, Item, Pat, Program, Stmt, TypeBody, TypeItem, TypeRef, UseItem,
};

pub fn format_program(program: &Program) -> String {
    let mut out = String::new();
    for (i, item) in program.items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        format_item(&mut out, item);
        out.push('\n');
    }
    out
}

fn format_item(out: &mut String, item: &Item) {
    match item {
        Item::Fn(f) => format_fn(out, f),
        Item::Type(t) => format_type(out, t),
        Item::Use(u) => format_use(out, u),
        Item::Extern(e) => {
            out.push_str("extern \"");
            out.push_str(&e.abi);
            out.push_str("\" {\n");
            for ef in &e.items {
                out.push_str("    fn ");
                out.push_str(&ef.name);
                out.push('(');
                for (i, p) in ef.params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&p.name);
                    if let Some(ty) = &p.ty {
                        out.push_str(": ");
                        format_type_ref(out, ty);
                    }
                }
                out.push(')');
                if let Some(ret) = &ef.ret {
                    out.push_str(" -> ");
                    format_type_ref(out, ret);
                }
                out.push('\n');
            }
            out.push('}');
        }
    }
}

fn format_use(out: &mut String, u: &UseItem) {
    out.push_str("use ");
    for (i, seg) in u.path.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(seg);
    }
}

fn format_fn(out: &mut String, f: &FnItem) {
    out.push_str("fn ");
    if let Some(r) = &f.receiver {
        out.push_str(r);
        out.push('.');
    }
    out.push_str(&f.name);
    out.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&p.name);
        if let Some(ty) = &p.ty {
            out.push_str(": ");
            format_type_ref(out, ty);
        }
    }
    out.push(')');
    if let Some(ret) = &f.ret {
        out.push_str(" -> ");
        format_type_ref(out, ret);
    }
    out.push(' ');
    format_block(out, &f.body, 0);
}

fn format_type(out: &mut String, t: &TypeItem) {
    out.push_str("type ");
    out.push_str(&t.name);
    out.push_str(" {\n");
    match &t.kind {
        TypeBody::Struct(fields) => {
            for f in fields {
                out.push_str("    ");
                out.push_str(&f.name);
                out.push_str(": ");
                format_type_ref(out, &f.ty);
                out.push_str(",\n");
            }
        }
        TypeBody::Adt(variants) => {
            for v in variants {
                out.push_str("    ");
                out.push_str(&v.name);
                if v.fields.is_empty() {
                    out.push_str(",\n");
                } else {
                    out.push_str(" { ");
                    for (i, f) in v.fields.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(&f.name);
                        out.push_str(": ");
                        format_type_ref(out, &f.ty);
                    }
                    out.push_str(" },\n");
                }
            }
        }
    }
    out.push('}');
}

fn format_type_ref(out: &mut String, t: &TypeRef) {
    match t {
        TypeRef::Named { name, args, .. } => {
            out.push_str(name);
            if !args.is_empty() {
                out.push('<');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    format_type_ref(out, a);
                }
                out.push('>');
            }
        }
        TypeRef::Ptr {
            is_const, inner, ..
        } => {
            out.push('*');
            if *is_const {
                out.push_str("const ");
            }
            format_type_ref(out, inner);
        }
    }
}

fn format_block(out: &mut String, b: &Block, indent: usize) {
    out.push_str("{\n");
    for stmt in &b.stmts {
        push_indent(out, indent + 1);
        format_stmt(out, stmt, indent + 1);
        out.push('\n');
    }
    push_indent(out, indent);
    out.push('}');
}

fn format_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    match stmt {
        Stmt::Let { name, ty, init, .. } => {
            out.push_str("let ");
            out.push_str(name);
            if let Some(t) = ty {
                out.push_str(": ");
                format_type_ref(out, t);
            }
            out.push_str(" = ");
            format_expr(out, init, indent);
        }
        Stmt::Expr(e) => format_expr(out, e, indent),
        Stmt::Return { value, .. } => {
            out.push_str("return");
            if let Some(v) = value {
                out.push(' ');
                format_expr(out, v, indent);
            }
        }
    }
}

fn format_expr(out: &mut String, e: &Expr, indent: usize) {
    match e {
        Expr::Ident { name, .. } => out.push_str(name),
        Expr::Int { value, .. } => out.push_str(&value.to_string()),
        Expr::Float { text, .. } => out.push_str(text),
        Expr::Str { value, .. } => {
            out.push('"');
            out.push_str(&escape(value));
            out.push('"');
        }
        Expr::CStr { value, .. } => {
            out.push_str("c\"");
            out.push_str(&escape(value));
            out.push('"');
        }
        Expr::Char { value, .. } => {
            out.push('\'');
            out.push(*value);
            out.push('\'');
        }
        Expr::Call { callee, args, .. } => {
            format_expr(out, callee, indent);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_expr(out, a, indent);
            }
            out.push(')');
        }
        Expr::Field { base, name, .. } => {
            format_expr(out, base, indent);
            out.push('.');
            out.push_str(name);
        }
        Expr::Index { base, index, .. } => {
            format_expr(out, base, indent);
            out.push('[');
            format_expr(out, index, indent);
            out.push(']');
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            format_expr(out, lhs, indent);
            out.push(' ');
            out.push_str(binop(op));
            out.push(' ');
            format_expr(out, rhs, indent);
        }
        Expr::Cast { expr, ty, .. } => {
            format_expr(out, expr, indent);
            out.push_str(" as ");
            format_type_ref(out, ty);
        }
        Expr::Is { expr, pat, .. } => {
            format_expr(out, expr, indent);
            out.push_str(" is ");
            format_pat(out, pat);
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            out.push_str("if ");
            format_expr(out, cond, indent);
            out.push(' ');
            format_block(out, then_block, indent);
            if let Some(eb) = else_block {
                out.push_str(" else ");
                format_block(out, eb, indent);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            out.push_str("match ");
            format_expr(out, scrutinee, indent);
            out.push_str(" {\n");
            for arm in arms {
                push_indent(out, indent + 1);
                format_pat(out, &arm.pat);
                out.push_str(" => ");
                format_expr(out, &arm.body, indent + 1);
                out.push_str(",\n");
            }
            push_indent(out, indent);
            out.push('}');
        }
        Expr::StructLit { name, fields, .. } => {
            out.push_str(name);
            out.push_str(" { ");
            for (i, (n, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(n);
                out.push_str(": ");
                format_expr(out, v, indent);
            }
            out.push_str(" }");
        }
        Expr::Task { body, .. } => {
            out.push_str("task ");
            format_block(out, body, indent);
        }
        Expr::Await { inner, .. } => {
            out.push_str("await ");
            format_expr(out, inner, indent);
        }
        Expr::Unsafe { body, .. } => {
            out.push_str("unsafe ");
            format_block(out, body, indent);
        }
        Expr::Group { inner, .. } => {
            out.push('(');
            format_expr(out, inner, indent);
            out.push(')');
        }
        Expr::Block { body, .. } => {
            format_block(out, body, indent);
        }
    }
}

fn format_pat(out: &mut String, p: &Pat) {
    match p {
        Pat::Ident { name, .. } => out.push_str(name),
        Pat::Call { name, args, .. } => {
            out.push_str(name);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_pat(out, a);
            }
            out.push(')');
        }
        Pat::Struct { name, fields, .. } => {
            out.push_str(name);
            out.push_str(" { ");
            for (i, (n, p)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(n);
                // shorthand if Ident same name
                match p {
                    Pat::Ident { name: bn, .. } if bn == n => {}
                    _ => {
                        out.push_str(": ");
                        format_pat(out, p);
                    }
                }
            }
            out.push_str(" }");
        }
    }
}

fn binop(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\0', "\\0")
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("    ");
    }
}
