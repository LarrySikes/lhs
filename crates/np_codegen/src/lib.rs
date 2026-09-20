//! Emit standalone C for a growing subset of LHS (v0.1+).
//! Full programs: use `lhsc run`. Cranelift is planned (DECISIONS D5).

use np_syntax::{BinOp, Block, Expr, FnItem, Item, Program, Stmt};
use std::fmt::Write as _;

#[derive(Debug)]
pub struct EmitError {
    pub message: String,
}

pub fn emit_c(program: &Program) -> Result<String, EmitError> {
    if !is_supported(program) {
        return Err(EmitError {
            message: "C backend supports fn/let/print/if/return/int/str/arithmetic (no ADT/match/methods yet). Use `lhsc run` for full LHS.".into(),
        });
    }

    let fns: Vec<&FnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Fn(f) if f.receiver.is_none() => Some(f),
            _ => None,
        })
        .collect();
    if !fns.iter().any(|f| f.name == "main") {
        return Err(EmitError {
            message: "no fn main".into(),
        });
    }

    let mut out = String::from(RUNTIME);
    out.push_str("\n/* --- LHS generated --- */\n\n");

    for f in &fns {
        let _ = writeln!(out, "static V {}(Env* e{});", c_name(f), c_params(f));
    }
    out.push('\n');

    for f in &fns {
        emit_fn(&mut out, f)?;
    }

    out.push_str(
        "int main(void) {\n    Env e; env_init(&e);\n    np_f_main(&e);\n    return 0;\n}\n",
    );
    Ok(out)
}

fn c_name(f: &FnItem) -> String {
    format!("np_f_{}", f.name)
}

fn c_params(f: &FnItem) -> String {
    let mut s = String::new();
    for p in &f.params {
        let _ = write!(s, ", V a_{}", p.name);
    }
    s
}

fn is_supported(program: &Program) -> bool {
    for item in &program.items {
        match item {
            Item::Type(_) | Item::Extern(_) => return false,
            Item::Fn(f) => {
                if f.receiver.is_some() {
                    return false;
                }
                for stmt in &f.body.stmts {
                    if !stmt_ok(stmt) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn stmt_ok(s: &Stmt) -> bool {
    match s {
        Stmt::Let { init, .. } => expr_ok(init),
        Stmt::Expr(e) => expr_ok(e),
        Stmt::Return { value, .. } => value.as_ref().map(|e| expr_ok(e)).unwrap_or(true),
    }
}

fn expr_ok(e: &Expr) -> bool {
    match e {
        Expr::Ident { .. } | Expr::Int { .. } | Expr::Str { .. } | Expr::Float { .. } => true,
        Expr::Binary { lhs, rhs, .. } => expr_ok(lhs) && expr_ok(rhs),
        Expr::Group { inner, .. } => expr_ok(inner),
        Expr::Cast { expr, .. } => expr_ok(expr),
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            expr_ok(cond)
                && then_block.stmts.iter().all(stmt_ok)
                && else_block
                    .as_ref()
                    .map(|b| b.stmts.iter().all(stmt_ok))
                    .unwrap_or(true)
        }
        Expr::Call { callee, args, .. } => {
            matches!(callee.as_ref(), Expr::Ident { .. }) && args.iter().all(expr_ok)
        }
        Expr::Task { body, .. } | Expr::Unsafe { body, .. } => body.stmts.iter().all(stmt_ok),
        Expr::Await { inner, .. } => expr_ok(inner),
        _ => false,
    }
}

fn emit_fn(out: &mut String, f: &FnItem) -> Result<(), EmitError> {
    let _ = writeln!(out, "static V {}(Env* e{}) {{", c_name(f), c_params(f));
    for p in &f.params {
        let _ = writeln!(out, "    env_set(e, \"{}\", a_{});", p.name, p.name);
    }
    let stmts = &f.body.stmts;
    if let Some((last, rest)) = stmts.split_last() {
        for stmt in rest {
            emit_stmt(out, stmt, 1)?;
        }
        match last {
            Stmt::Expr(e) => {
                out.push_str("    return ");
                emit_expr(out, e)?;
                out.push_str(";\n");
            }
            Stmt::Return { value, .. } => {
                out.push_str("    return ");
                match value {
                    Some(v) => emit_expr(out, v)?,
                    None => out.push_str("V_unit()"),
                }
                out.push_str(";\n");
            }
            other => {
                emit_stmt(out, other, 1)?;
                out.push_str("    return V_unit();\n");
            }
        }
    } else {
        out.push_str("    return V_unit();\n");
    }
    out.push_str("}\n\n");
    Ok(())
}

fn emit_block(out: &mut String, b: &Block, indent: usize) -> Result<(), EmitError> {
    for stmt in &b.stmts {
        emit_stmt(out, stmt, indent)?;
    }
    Ok(())
}

fn emit_stmt(out: &mut String, stmt: &Stmt, indent: usize) -> Result<(), EmitError> {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let { name, init, .. } => {
            let _ = write!(out, "{pad}{{ V t = ");
            emit_expr(out, init)?;
            let _ = writeln!(out, "; env_set(e, \"{name}\", t); }}");
        }
        Stmt::Expr(e) => {
            let _ = write!(out, "{pad}(void)(");
            emit_expr(out, e)?;
            let _ = writeln!(out, ");");
        }
        Stmt::Return { value, .. } => {
            let _ = write!(out, "{pad}return ");
            match value {
                Some(v) => emit_expr(out, v)?,
                None => out.push_str("V_unit()"),
            }
            let _ = writeln!(out, ";");
        }
    }
    Ok(())
}

fn emit_expr(out: &mut String, e: &Expr) -> Result<(), EmitError> {
    match e {
        Expr::Ident { name, .. } => {
            let _ = write!(out, "env_get(e, \"{name}\")");
        }
        Expr::Int { value, .. } => {
            let _ = write!(out, "V_int({value})");
        }
        Expr::Float { text, .. } => {
            let _ = write!(out, "V_float({text})");
        }
        Expr::Str { value, .. } => {
            let esc = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            let _ = write!(out, "V_str(\"{esc}\")");
        }
        Expr::Group { inner, .. } => {
            out.push('(');
            emit_expr(out, inner)?;
            out.push(')');
        }
        Expr::Cast { expr, .. } => emit_expr(out, expr)?,
        Expr::Binary { op, lhs, rhs, .. } => {
            let _ = write!(out, "V_bin({}, ", bin_code(*op));
            emit_expr(out, lhs)?;
            out.push_str(", ");
            emit_expr(out, rhs)?;
            out.push(')');
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            // GNU statement expressions
            out.push_str("({ V _c = ");
            emit_expr(out, cond)?;
            out.push_str("; V _r = V_unit(); if (_c.b) { ");
            for stmt in &then_block.stmts {
                match stmt {
                    Stmt::Let { name, init, .. } => {
                        out.push_str("{ V t = ");
                        emit_expr(out, init)?;
                        let _ = write!(out, "; env_set(e, \"{name}\", t); }} ");
                    }
                    Stmt::Expr(ex) => {
                        out.push_str("_r = ");
                        emit_expr(out, ex)?;
                        out.push_str("; ");
                    }
                    Stmt::Return { value, .. } => {
                        out.push_str("return ");
                        match value {
                            Some(v) => emit_expr(out, v)?,
                            None => out.push_str("V_unit()"),
                        }
                        out.push_str("; ");
                    }
                }
            }
            out.push_str("} else { ");
            if let Some(eb) = else_block {
                for stmt in &eb.stmts {
                    match stmt {
                        Stmt::Let { name, init, .. } => {
                            out.push_str("{ V t = ");
                            emit_expr(out, init)?;
                            let _ = write!(out, "; env_set(e, \"{name}\", t); }} ");
                        }
                        Stmt::Expr(ex) => {
                            out.push_str("_r = ");
                            emit_expr(out, ex)?;
                            out.push_str("; ");
                        }
                        Stmt::Return { value, .. } => {
                            out.push_str("return ");
                            match value {
                                Some(v) => emit_expr(out, v)?,
                                None => out.push_str("V_unit()"),
                            }
                            out.push_str("; ");
                        }
                    }
                }
            }
            out.push_str("} _r; })");
        }
        Expr::Call { callee, args, .. } => {
            let Expr::Ident { name, .. } = callee.as_ref() else {
                return Err(EmitError {
                    message: "call target must be a name".into(),
                });
            };
            if name == "print" {
                out.push_str("V_print(");
                if let Some(a) = args.first() {
                    emit_expr(out, a)?;
                } else {
                    out.push_str("V_unit()");
                }
                out.push(')');
                return Ok(());
            }
            let _ = write!(out, "np_f_{name}(e");
            for a in args {
                out.push_str(", ");
                emit_expr(out, a)?;
            }
            out.push(')');
        }
        Expr::Task { body, .. } | Expr::Unsafe { body, .. } => {
            out.push_str("({ V _r = V_unit(); ");
            for stmt in &body.stmts {
                if let Stmt::Expr(ex) = stmt {
                    out.push_str("_r = ");
                    emit_expr(out, ex)?;
                    out.push_str("; ");
                } else {
                    emit_stmt(out, stmt, 0)?;
                }
            }
            out.push_str("_r; })");
        }
        Expr::Await { inner, .. } => emit_expr(out, inner)?,
        _ => {
            return Err(EmitError {
                message: "unsupported expression in C backend".into(),
            })
        }
    }
    Ok(())
}

fn bin_code(op: BinOp) -> i32 {
    match op {
        BinOp::Add => 1,
        BinOp::Sub => 2,
        BinOp::Mul => 3,
        BinOp::Div => 4,
        BinOp::Eq => 5,
        BinOp::Ne => 6,
        BinOp::Lt => 7,
        BinOp::Le => 8,
        BinOp::Gt => 9,
        BinOp::Ge => 10,
        BinOp::And => 11,
        BinOp::Or => 12,
    }
}

pub fn compile_c_to_binary(c_source: &str, out_path: &str) -> Result<(), EmitError> {
    let tmp = format!("{out_path}.lhs.c");
    std::fs::write(&tmp, c_source).map_err(|e| EmitError {
        message: e.to_string(),
    })?;
    let status = std::process::Command::new("cc")
        .args(["-O2", "-std=gnu11", "-o", out_path, &tmp, "-lm"])
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

const RUNTIME: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef struct { enum { U, B, I, F, S } k; bool b; long i; double f; char* s; } V;
typedef struct { char* keys[256]; V vals[256]; int n; } Env;
static void env_init(Env* e){ e->n=0; }
static void env_set(Env* e, const char* k, V v){
  for(int i=0;i<e->n;i++) if(!strcmp(e->keys[i],k)){ e->vals[i]=v; return; }
  e->keys[e->n]=strdup(k); e->vals[e->n++]=v;
}
static V env_get(Env* e, const char* k){
  for(int i=0;i<e->n;i++) if(!strcmp(e->keys[i],k)) return e->vals[i];
  fprintf(stderr,"undefined %s\n",k); exit(1);
}
static V V_unit(void){ V v={.k=U}; return v; }
static V V_int(long i){ V v={.k=I,.i=i}; return v; }
static V V_float(double f){ V v={.k=F,.f=f}; return v; }
static V V_str(const char* s){ V v={.k=S,.s=strdup(s)}; return v; }
static V V_bool(bool b){ V v={.k=B,.b=b}; return v; }
static V V_print(V v){
  if(v.k==S) printf("%s\n", v.s);
  else if(v.k==I) printf("%ld\n", v.i);
  else if(v.k==F) printf("%g\n", v.f);
  else if(v.k==B) printf("%s\n", v.b?"true":"false");
  else printf("\n");
  return V_unit();
}
static V V_bin(int op, V a, V b){
  if(op>=5 && op<=10){
    if(a.k==S && b.k==S){ int c=strcmp(a.s,b.s); if(op==5) return V_bool(c==0); if(op==6) return V_bool(c!=0); }
    long x=a.k==I?a.i:(long)a.f, y=b.k==I?b.i:(long)b.f;
    if(op==5) return V_bool(x==y); if(op==6) return V_bool(x!=y);
    if(op==7) return V_bool(x<y); if(op==8) return V_bool(x<=y);
    if(op==9) return V_bool(x>y); if(op==10) return V_bool(x>=y);
  }
  if(op==11) return V_bool(a.b&&b.b); if(op==12) return V_bool(a.b||b.b);
  if(a.k==F||b.k==F){
    double x=a.k==F?a.f:(double)a.i, y=b.k==F?b.f:(double)b.i;
    if(op==1) return V_float(x+y); if(op==2) return V_float(x-y);
    if(op==3) return V_float(x*y); if(op==4) return V_float(x/y);
  }
  long x=a.i,y=b.i;
  if(op==1) return V_int(x+y); if(op==2) return V_int(x-y);
  if(op==3) return V_int(x*y); if(op==4) return V_int(x/y);
  return V_unit();
}
"#;
