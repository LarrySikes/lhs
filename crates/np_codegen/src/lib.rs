//! Emit standalone C for a supported subset of `.np` (v0.1).
//! Full programs: use `npc run`. Cranelift is planned (DECISIONS D5).

use np_syntax::{BinOp, Expr, FnItem, Item, Program, Stmt};
use std::fmt::Write as _;

#[derive(Debug)]
pub struct EmitError {
    pub message: String,
}

pub fn emit_c(program: &Program) -> Result<String, EmitError> {
    // Only support: fn main with let/print/int/str/binary — enough for 01/02.
    // Broader programs should use the interpreter.
    if !is_simple(program) {
        return Err(EmitError {
            message: "C backend supports a simple subset (main + let/print/literals/arithmetic). Use `npc run` for full programs.".into(),
        });
    }
    let main = program.items.iter().find_map(|i| match i {
        Item::Fn(f) if f.name == "main" && f.receiver.is_none() => Some(f),
        _ => None,
    });
    let Some(main) = main else {
        return Err(EmitError {
            message: "no fn main".into(),
        });
    };

    let mut body = String::new();
    for stmt in &main.body.stmts {
        emit_stmt(&mut body, stmt, 1)?;
    }

    Ok(format!(
        r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {{ int is_str; long i; char* s; }} V;
static V V_int(long i) {{ V v; v.is_str=0; v.i=i; v.s=NULL; return v; }}
static V V_str(const char* s) {{ V v; v.is_str=1; v.i=0; v.s=strdup(s); return v; }}
static void np_print(V v) {{ if (v.is_str) printf("%s\n", v.s); else printf("%ld\n", v.i); }}

int main(void) {{
{body}    return 0;
}}
"#
    ))
}

fn is_simple(program: &Program) -> bool {
    for item in &program.items {
        match item {
            Item::Fn(f) if f.name == "main" && f.receiver.is_none() && f.params.is_empty() => {
                for stmt in &f.body.stmts {
                    if !stmt_simple(stmt) {
                        return false;
                    }
                }
            }
            Item::Fn(_) | Item::Type(_) | Item::Extern(_) => return false,
        }
    }
    true
}

fn stmt_simple(s: &Stmt) -> bool {
    match s {
        Stmt::Let { init, .. } => expr_simple(init),
        Stmt::Expr(e) => expr_simple(e),
        Stmt::Return { .. } => false,
    }
}

fn expr_simple(e: &Expr) -> bool {
    match e {
        Expr::Ident { .. } | Expr::Int { .. } | Expr::Str { .. } => true,
        Expr::Binary { lhs, rhs, .. } => expr_simple(lhs) && expr_simple(rhs),
        Expr::Group { inner, .. } => expr_simple(inner),
        Expr::Call { callee, args, .. } => {
            matches!(&**callee, Expr::Ident { name, .. } if name == "print")
                && args.iter().all(expr_simple)
        }
        _ => false,
    }
}

fn emit_stmt(out: &mut String, stmt: &Stmt, indent: usize) -> Result<(), EmitError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let { name, init, .. } => {
            let _ = write!(out, "{pad}V v_{name} = ");
            emit_expr(out, init)?;
            let _ = writeln!(out, ";");
        }
        Stmt::Expr(e) => {
            if let Expr::Call { callee, args, .. } = e {
                if let Expr::Ident { name, .. } = callee.as_ref() {
                    if name == "print" && args.len() == 1 {
                        let _ = write!(out, "{pad}np_print(");
                        emit_expr(out, &args[0])?;
                        let _ = writeln!(out, ");");
                        return Ok(());
                    }
                }
            }
            return Err(EmitError {
                message: "unsupported statement in C subset".into(),
            });
        }
        _ => {
            return Err(EmitError {
                message: "unsupported statement in C subset".into(),
            })
        }
    }
    Ok(())
}

fn emit_expr(out: &mut String, e: &Expr) -> Result<(), EmitError> {
    match e {
        Expr::Ident { name, .. } => {
            let _ = write!(out, "v_{name}");
        }
        Expr::Int { value, .. } => {
            let _ = write!(out, "V_int({value})");
        }
        Expr::Str { value, .. } => {
            let esc = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            let _ = write!(out, "V_str(\"{esc}\")");
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            // only int arithmetic in subset
            out.push_str("V_int(");
            emit_expr_int(out, lhs)?;
            out.push(' ');
            out.push_str(match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                _ => {
                    return Err(EmitError {
                        message: "unsupported op in C subset".into(),
                    })
                }
            });
            out.push(' ');
            emit_expr_int(out, rhs)?;
            out.push(')');
        }
        Expr::Group { inner, .. } => {
            out.push('(');
            emit_expr(out, inner)?;
            out.push(')');
        }
        _ => {
            return Err(EmitError {
                message: "unsupported expr in C subset".into(),
            })
        }
    }
    Ok(())
}

fn emit_expr_int(out: &mut String, e: &Expr) -> Result<(), EmitError> {
    match e {
        Expr::Ident { name, .. } => {
            let _ = write!(out, "v_{name}.i");
        }
        Expr::Int { value, .. } => {
            let _ = write!(out, "{value}");
        }
        Expr::Binary { .. } | Expr::Group { .. } => {
            out.push('(');
            // emit as V then .i
            emit_expr(out, e)?;
            out.push_str(").i");
        }
        _ => {
            return Err(EmitError {
                message: "expected int expr".into(),
            })
        }
    }
    Ok(())
}

pub fn compile_c_to_binary(c_source: &str, out_path: &str) -> Result<(), EmitError> {
    let tmp = format!("{out_path}.np.c");
    std::fs::write(&tmp, c_source).map_err(|e| EmitError {
        message: e.to_string(),
    })?;
    let status = std::process::Command::new("cc")
        .args(["-O2", "-o", out_path, &tmp])
        .status()
        .map_err(|e| EmitError {
            message: format!("failed to run cc: {e}"),
        })?;
    if !status.success() {
        return Err(EmitError {
            message: format!("cc failed for {tmp}"),
        });
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

// silence unused import in some builds
#[allow(dead_code)]
fn _use_fnitem(_: &FnItem) {}
