//! Tree-walking interpreter for `.lhs` programs.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use np_syntax::{BinOp, Block, Expr, FnItem, Item, Pat, Program, Stmt, TypeItem, TypeRef};

/// Handle for a parallel `task { ... }` (join once via `await`).
#[derive(Debug)]
pub struct TaskHandle {
    inner: Mutex<Option<JoinHandle<Result<Value>>>>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Char(char),
    Variant {
        name: String,
        fields: HashMap<String, Value>,
        positional: Vec<Value>,
    },
    /// Parallel task; `await` joins. Shared via Arc so env can clone.
    Task(Arc<TaskHandle>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Char(c) => write!(f, "{c}"),
            Value::Task(_) => write!(f, "<task>"),
            Value::Variant {
                name,
                fields,
                positional,
            } => {
                if !positional.is_empty() {
                    write!(f, "{name}(")?;
                    for (i, v) in positional.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{v}")?;
                    }
                    write!(f, ")")
                } else if fields.is_empty() {
                    write!(f, "{name}")
                } else {
                    write!(f, "{name} {{ ... }}")
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum EvalError {
    Runtime(String),
    Return(Value),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Runtime(m) => write!(f, "{m}"),
            EvalError::Return(_) => write!(f, "uncaught return"),
        }
    }
}

impl std::error::Error for EvalError {}

type Result<T> = std::result::Result<T, EvalError>;

fn err(msg: impl Into<String>) -> EvalError {
    EvalError::Runtime(msg.into())
}

struct Interpreter<'a, W: Write> {
    program: &'a Program,
    #[allow(dead_code)]
    types: HashMap<String, &'a TypeItem>,
    out: W,
    /// Shared program for worker threads (same contents as `program`).
    program_arc: Arc<Program>,
}

impl<'a, W: Write> Interpreter<'a, W> {
    fn find_fn(&self, receiver: Option<&str>, name: &str) -> Option<&'a FnItem> {
        self.program.items.iter().find_map(|item| match item {
            Item::Fn(f) if f.receiver.as_deref() == receiver && f.name == name => Some(f),
            _ => None,
        })
    }

    fn run_main(&mut self) -> Result<Value> {
        let main = self
            .find_fn(None, "main")
            .ok_or_else(|| err("no `fn main` found"))?;
        if !main.params.is_empty() {
            return Err(err("`main` must take no parameters"));
        }
        self.call_fn(main, Vec::new())
    }

    fn call_fn(&mut self, f: &FnItem, args: Vec<Value>) -> Result<Value> {
        let mut env = HashMap::new();
        if f.params.len() != args.len() {
            return Err(err(format!(
                "function `{}` expected {} args, got {}",
                f.name,
                f.params.len(),
                args.len()
            )));
        }
        for (p, a) in f.params.iter().zip(args) {
            env.insert(p.name.clone(), a);
        }
        match self.eval_block(&mut env, &f.body) {
            Err(EvalError::Return(v)) => Ok(v),
            other => other,
        }
    }

    fn eval_block(&mut self, env: &mut HashMap<String, Value>, block: &Block) -> Result<Value> {
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            last = self.eval_stmt(env, stmt)?;
        }
        Ok(last)
    }

    fn eval_stmt(&mut self, env: &mut HashMap<String, Value>, stmt: &Stmt) -> Result<Value> {
        match stmt {
            Stmt::Let { name, init, .. } => {
                let v = self.eval_expr(env, init)?;
                env.insert(name.clone(), v);
                Ok(Value::Unit)
            }
            Stmt::Expr(e) => self.eval_expr(env, e),
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval_expr(env, e)?,
                    None => Value::Unit,
                };
                Err(EvalError::Return(v))
            }
        }
    }

    fn eval_expr(&mut self, env: &mut HashMap<String, Value>, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Ident { name, .. } => {
                if let Some(v) = env.get(name) {
                    return Ok(v.clone());
                }
                if name == "None" || name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    return Ok(Value::Variant {
                        name: name.clone(),
                        fields: HashMap::new(),
                        positional: Vec::new(),
                    });
                }
                Err(err(format!("undefined variable `{name}`")))
            }
            Expr::Int { value, .. } => Ok(Value::Int(*value)),
            Expr::Float { text, .. } => Ok(Value::Float(
                text.parse()
                    .map_err(|_| err(format!("bad float `{text}`")))?,
            )),
            Expr::Str { value, .. } | Expr::CStr { value, .. } => Ok(Value::Str(value.clone())),
            Expr::Char { value, .. } => Ok(Value::Char(*value)),
            Expr::Group { inner, .. } => self.eval_expr(env, inner),
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = self.eval_expr(env, lhs)?;
                let r = self.eval_expr(env, rhs)?;
                eval_binary(*op, &l, &r)
            }
            Expr::Cast { expr, ty, .. } => {
                let v = self.eval_expr(env, expr)?;
                cast_value(v, ty)
            },
            Expr::Is { expr, pat, .. } => {
                let v = self.eval_expr(env, expr)?;
                Ok(Value::Bool(match_pat(&v, pat, &mut HashMap::new())))
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                if truthy(&self.eval_expr(env, cond)?)? {
                    self.eval_block(env, then_block)
                } else if let Some(eb) = else_block {
                    self.eval_block(env, eb)
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let v = self.eval_expr(env, scrutinee)?;
                for arm in arms {
                    let mut binds = HashMap::new();
                    if match_pat(&v, &arm.pat, &mut binds) {
                        for (k, val) in binds {
                            env.insert(k, val);
                        }
                        return self.eval_expr(env, &arm.body);
                    }
                }
                Err(err("non-exhaustive match"))
            }
            Expr::StructLit { name, fields, .. } => {
                let mut map = HashMap::new();
                for (k, e) in fields {
                    map.insert(k.clone(), self.eval_expr(env, e)?);
                }
                Ok(Value::Variant {
                    name: name.clone(),
                    fields: map,
                    positional: Vec::new(),
                })
            }
            Expr::Field { base, name, .. } => match self.eval_expr(env, base)? {
                Value::Variant { fields, .. } => fields
                    .get(name)
                    .cloned()
                    .ok_or_else(|| err(format!("no field `{name}`"))),
                Value::Str(s) if name == "len" => Ok(Value::Int(s.chars().count() as i64)),
                other => Err(err(format!("cannot read field `{name}` on {other}"))),
            },
            Expr::Index { base, index, .. } => {
                let b = self.eval_expr(env, base)?;
                let i = self.eval_expr(env, index)?;
                match (b, i) {
                    (Value::Str(s), Value::Int(n)) => s
                        .chars()
                        .nth(n as usize)
                        .map(Value::Char)
                        .ok_or_else(|| err("string index out of range")),
                    _ => Err(err("index requires string[int]")),
                }
            }
            Expr::Call { callee, args, .. } => self.eval_call(env, callee, args),
            Expr::Block { body, .. } | Expr::Unsafe { body, .. } => self.eval_block(env, body),
            Expr::Task { body, .. } => {
                let program = Arc::clone(&self.program_arc);
                let body = body.clone();
                let handle = thread::spawn(move || {
                    let mut types = HashMap::new();
                    for item in &program.items {
                        if let Item::Type(t) = item {
                            types.insert(t.name.clone(), t);
                        }
                    }
                    let mut worker = Interpreter {
                        program: &program,
                        types,
                        out: io::sink(),
                        program_arc: Arc::clone(&program),
                    };
                    let mut local = HashMap::new();
                    match worker.eval_block(&mut local, &body) {
                        Err(EvalError::Return(v)) => Ok(v),
                        other => other,
                    }
                });
                Ok(Value::Task(Arc::new(TaskHandle {
                    inner: Mutex::new(Some(handle)),
                })))
            }
            Expr::Await { inner, .. } => {
                let v = self.eval_expr(env, inner)?;
                match v {
                    Value::Task(t) => {
                        let handle = t
                            .inner
                            .lock()
                            .map_err(|_| err("task lock poisoned"))?
                            .take()
                            .ok_or_else(|| err("task already awaited"))?;
                        handle
                            .join()
                            .map_err(|_| err("task panicked"))?
                    }
                    other => Ok(other),
                }
            }
        }
    }

    fn eval_call(
        &mut self,
        env: &mut HashMap<String, Value>,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<Value> {
        let mut arg_vals = Vec::with_capacity(args.len());
        for a in args {
            arg_vals.push(self.eval_expr(env, a)?);
        }

        if let Expr::Field {
            base,
            name: method,
            ..
        } = callee
        {
            let recv = self.eval_expr(env, base)?;
            match method.as_str() {
                "len" => {
                    return match recv {
                        Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                        _ => Err(err(".len() requires string")),
                    }
                }
                "sqrt" => {
                    return match recv {
                        Value::Float(x) => Ok(Value::Float(x.sqrt())),
                        Value::Int(n) => Ok(Value::Float((n as f64).sqrt())),
                        _ => Err(err(".sqrt() requires number")),
                    }
                }
                "unwrap" => {
                    return match recv {
                        Value::Variant {
                            name,
                            positional,
                            fields,
                        } if name == "Some" || name == "Ok" => positional
                            .first()
                            .cloned()
                            .or_else(|| fields.values().next().cloned())
                            .ok_or_else(|| err("unwrap on empty")),
                        _ => Err(err("unwrap on non-Some/Ok")),
                    }
                }
                _ => {}
            }

            let type_name = match &recv {
                Value::Variant { name, .. } => name.clone(),
                _ => return Err(err(format!("no method `{method}`"))),
            };
            if let Some(f) = self.find_fn(Some(&type_name), method) {
                let mut all = vec![recv];
                all.extend(arg_vals);
                return self.call_fn(f, all);
            }
            return Err(err(format!("no method `{type_name}.{method}`")));
        }

        if let Expr::Ident { name, .. } = callee {
            if name == "print" {
                let mut first = true;
                for v in &arg_vals {
                    if !first {
                        write!(self.out, " ").map_err(|e| err(e.to_string()))?;
                    }
                    first = false;
                    write!(self.out, "{v}").map_err(|e| err(e.to_string()))?;
                }
                writeln!(self.out).map_err(|e| err(e.to_string()))?;
                return Ok(Value::Unit);
            }
            if name == "read_file" {
                let path = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("read_file expects str path")),
                };
                match std::fs::read_to_string(&path) {
                    Ok(s) => {
                        return Ok(Value::Variant {
                            name: "Ok".into(),
                            fields: HashMap::new(),
                            positional: vec![Value::Str(s)],
                        })
                    }
                    Err(e) => {
                        return Ok(Value::Variant {
                            name: "Err".into(),
                            fields: HashMap::new(),
                            positional: vec![Value::Str(e.to_string())],
                        })
                    }
                }
            }
            if name == "write_file" {
                let path = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("write_file expects str path")),
                };
                let contents = match arg_vals.get(1) {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("write_file expects str contents")),
                };
                match std::fs::write(&path, contents) {
                    Ok(()) => {
                        return Ok(Value::Variant {
                            name: "Ok".into(),
                            fields: HashMap::new(),
                            positional: vec![Value::Unit],
                        })
                    }
                    Err(e) => {
                        return Ok(Value::Variant {
                            name: "Err".into(),
                            fields: HashMap::new(),
                            positional: vec![Value::Str(e.to_string())],
                        })
                    }
                }
            }
            if name == "getenv" {
                let key = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("getenv expects str")),
                };
                return Ok(match std::env::var(&key) {
                    Ok(s) => Value::Variant {
                        name: "Some".into(),
                        fields: HashMap::new(),
                        positional: vec![Value::Str(s)],
                    },
                    Err(_) => Value::Variant {
                        name: "None".into(),
                        fields: HashMap::new(),
                        positional: Vec::new(),
                    },
                });
            }
            if name == "argc" {
                return Ok(Value::Int(std::env::args().len() as i64));
            }
            if name == "arg" {
                let i = match arg_vals.first() {
                    Some(Value::Int(n)) => *n,
                    _ => return Err(err("arg expects int index")),
                };
                return Ok(match std::env::args().nth(i as usize) {
                    Some(s) => Value::Variant {
                        name: "Some".into(),
                        fields: HashMap::new(),
                        positional: vec![Value::Str(s)],
                    },
                    None => Value::Variant {
                        name: "None".into(),
                        fields: HashMap::new(),
                        positional: Vec::new(),
                    },
                });
            }
            if name == "exit" {
                let code = match arg_vals.first() {
                    Some(Value::Int(n)) => *n as i32,
                    _ => 0,
                };
                std::process::exit(code);
            }
            if name == "sleep_ms" {
                let ms = match arg_vals.first() {
                    Some(Value::Int(n)) => *n,
                    _ => return Err(err("sleep_ms expects int")),
                };
                if ms > 0 {
                    thread::sleep(std::time::Duration::from_millis(ms as u64));
                }
                return Ok(Value::Unit);
            }
            if name == "now_ms" {
                use std::time::{SystemTime, UNIX_EPOCH};
                let ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                return Ok(Value::Int(ms));
            }
            if name == "eprint" {
                for v in &arg_vals {
                    eprint!("{v}");
                }
                eprintln!();
                return Ok(Value::Unit);
            }
            if name == "gc" {
                let n = unsafe { lhs_mem::lhs_gc() };
                return Ok(Value::Int(n as i64));
            }
            if name == "abs" {
                return match arg_vals.first() {
                    Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
                    Some(Value::Float(x)) => Ok(Value::Float(x.abs())),
                    _ => Err(err("abs expects int or float")),
                };
            }
            if name == "min" || name == "max" {
                let a = arg_vals.first().ok_or_else(|| err("min/max need 2 args"))?;
                let b = arg_vals.get(1).ok_or_else(|| err("min/max need 2 args"))?;
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => {
                        return Ok(Value::Int(if name == "min" {
                            (*x).min(*y)
                        } else {
                            (*x).max(*y)
                        }));
                    }
                    (Value::Float(x), Value::Float(y)) => {
                        return Ok(Value::Float(if name == "min" {
                            x.min(*y)
                        } else {
                            x.max(*y)
                        }));
                    }
                    _ => return Err(err("min/max expect matching numeric types")),
                }
            }
            if name == "assert" {
                match arg_vals.first() {
                    Some(Value::Bool(true)) => return Ok(Value::Unit),
                    Some(Value::Bool(false)) => {
                        let msg = arg_vals
                            .get(1)
                            .map(|v| format!("{v}"))
                            .unwrap_or_else(|| "assertion failed".into());
                        return Err(err(msg));
                    }
                    _ => return Err(err("assert expects bool")),
                }
            }
            if name == "read_line" {
                let mut line = String::new();
                match io::stdin().read_line(&mut line) {
                    Ok(0) => {
                        return Ok(Value::Variant {
                            name: "None".into(),
                            fields: HashMap::new(),
                            positional: vec![],
                        })
                    }
                    Ok(_) => {
                        while line.ends_with('\n') || line.ends_with('\r') {
                            line.pop();
                        }
                        return Ok(Value::Variant {
                            name: "Some".into(),
                            fields: HashMap::new(),
                            positional: vec![Value::Str(line)],
                        });
                    }
                    Err(e) => return Err(err(format!("read_line: {e}"))),
                }
            }
            if name == "str_slice" {
                let s = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("str_slice expects str")),
                };
                let start = match arg_vals.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => return Err(err("str_slice start expects i32")),
                };
                let end = match arg_vals.get(2) {
                    Some(Value::Int(n)) => *n,
                    _ => return Err(err("str_slice end expects i32")),
                };
                let chars: Vec<char> = s.chars().collect();
                let n = chars.len() as i64;
                let a = start.clamp(0, n) as usize;
                let b = end.clamp(0, n) as usize;
                let b = b.max(a);
                return Ok(Value::Str(chars[a..b].iter().collect()));
            }
            if name == "find_char" {
                let s = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(err("find_char expects str")),
                };
                let ch = match arg_vals.get(1) {
                    Some(Value::Char(c)) => *c,
                    _ => return Err(err("find_char expects char")),
                };
                let from = match arg_vals.get(2) {
                    Some(Value::Int(n)) => *n,
                    None => 0,
                    _ => return Err(err("find_char from expects i32")),
                };
                for (i, c) in s.chars().enumerate() {
                    if (i as i64) >= from && c == ch {
                        return Ok(Value::Int(i as i64));
                    }
                }
                return Ok(Value::Int(-1));
            }
            if matches!(name.as_str(), "Some" | "Ok" | "Err") {
                return Ok(Value::Variant {
                    name: name.clone(),
                    fields: HashMap::new(),
                    positional: arg_vals,
                });
            }
            if let Some(f) = self.find_fn(None, name) {
                return self.call_fn(f, arg_vals);
            }
            // Interpreter stubs for common extern decls (no real FFI yet)
            if name == "puts" {
                if let Some(Value::Str(s)) = arg_vals.first() {
                    let s = s.trim_end_matches('\0');
                    writeln!(self.out, "{s}").map_err(|e| err(e.to_string()))?;
                    return Ok(Value::Int(0));
                }
                return Err(err("puts expects a string"));
            }
            return Err(err(format!("undefined function `{name}`")));
        }

        Err(err("call target must be a name or method"))
    }
}

fn truthy(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(err(format!("condition is not bool: {v}"))),
    }
}

fn cast_value(v: Value, ty: &TypeRef) -> Result<Value> {
    let TypeRef::Named { name, .. } = ty else {
        return Ok(v);
    };
    match name.as_str() {
        "i32" | "i64" | "int" => match v {
            Value::Int(n) => Ok(Value::Int(n)),
            Value::Char(c) => Ok(Value::Int(c as i64)),
            Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
            Value::Float(f) => Ok(Value::Int(f as i64)),
            other => Ok(other),
        },
        "f64" | "float" => match v {
            Value::Float(f) => Ok(Value::Float(f)),
            Value::Int(n) => Ok(Value::Float(n as f64)),
            other => Ok(other),
        },
        "char" => match v {
            Value::Char(c) => Ok(Value::Char(c)),
            Value::Int(n) => {
                let c = char::from_u32(n as u32).unwrap_or('\0');
                Ok(Value::Char(c))
            }
            other => Ok(other),
        },
        "str" => match v {
            Value::Str(s) => Ok(Value::Str(s)),
            Value::Char(c) => Ok(Value::Str(c.to_string())),
            Value::Int(n) => Ok(Value::Str(n.to_string())),
            Value::Float(f) => Ok(Value::Str(f.to_string())),
            other => Ok(other),
        },
        _ => Ok(v),
    }
}

fn eval_binary(op: BinOp, l: &Value, r: &Value) -> Result<Value> {
    use BinOp::*;
    match (op, l, r) {
        (Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (Add, Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Add, Value::Str(a), Value::Char(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Add, Value::Char(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Add, Value::Str(a), Value::Int(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Add, Value::Int(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (Div, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
        (Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
        (Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
        (Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
        (Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
        (Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
        (Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
        (Div, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),
        (Div, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
        (Eq, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a == b)),
        (Ne, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a != b)),
        (Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        (Le, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        (Eq, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a == b)),
        (Ne, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a != b)),
        (Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
        (Le, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
        (Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
        (Ge, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
        (Eq, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a == *b as f64)),
        (Ne, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a != *b as f64)),
        (Lt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a < *b as f64)),
        (Le, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a <= *b as f64)),
        (Gt, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a > *b as f64)),
        (Ge, Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a >= *b as f64)),
        (Eq, Value::Int(a), Value::Float(b)) => Ok(Value::Bool(*a as f64 == *b)),
        (Ne, Value::Int(a), Value::Float(b)) => Ok(Value::Bool(*a as f64 != *b)),
        (Lt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) < *b)),
        (Le, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) <= *b)),
        (Gt, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) > *b)),
        (Ge, Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) >= *b)),
        (Eq, Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a == b)),
        (Ne, Value::Str(a), Value::Str(b)) => Ok(Value::Bool(a != b)),
        (Eq, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a == b)),
        (Ne, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a != b)),
        (Lt, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a < b)),
        (Le, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a <= b)),
        (Gt, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a > b)),
        (Ge, Value::Char(a), Value::Char(b)) => Ok(Value::Bool(a >= b)),
        (And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
        (Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
        (Sub, Value::Char(a), Value::Char(b)) => Ok(Value::Int(*a as i64 - *b as i64)),
        (Sub, Value::Int(a), Value::Char(b)) => Ok(Value::Int(a - *b as i64)),
        (Sub, Value::Char(a), Value::Int(b)) => Ok(Value::Int(*a as i64 - b)),
        _ => Err(err(format!("unsupported op {op:?} on {l} and {r}"))),
    }
}

fn match_pat(v: &Value, pat: &Pat, binds: &mut HashMap<String, Value>) -> bool {
    match pat {
        Pat::Ident { name, .. } => {
            if name == "None" {
                matches!(
                    v,
                    Value::Variant {
                        name: n,
                        positional,
                        fields
                    } if n == "None" && positional.is_empty() && fields.is_empty()
                )
            } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                matches!(v, Value::Variant { name: n, .. } if n == name)
            } else {
                binds.insert(name.clone(), v.clone());
                true
            }
        }
        Pat::Call { name, args, .. } => match v {
            Value::Variant {
                name: vn,
                positional,
                ..
            } if vn == name && positional.len() == args.len() => {
                for (p, a) in positional.iter().zip(args) {
                    if !match_pat(p, a, binds) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
        Pat::Struct { name, fields, .. } => match v {
            Value::Variant {
                name: vn,
                fields: vf,
                ..
            } if vn == name => {
                for (fname, fpat) in fields {
                    let Some(fv) = vf.get(fname) else {
                        return false;
                    };
                    if !match_pat(fv, fpat, binds) {
                        return false;
                    }
                }
                true
            }
            _ => false,
        },
    }
}

pub fn run_program(program: &Program, out: impl Write) -> Result<Value> {
    let program_arc = Arc::new(program.clone());
    let mut types = HashMap::new();
    for item in &program_arc.items {
        if let Item::Type(t) = item {
            types.insert(t.name.clone(), t);
        }
    }
    let mut interp = Interpreter {
        program: &program_arc,
        types,
        out,
        program_arc: Arc::clone(&program_arc),
    };
    interp.run_main()
}

pub fn run_program_stdout(program: &Program) -> Result<Value> {
    run_program(program, io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;
    use np_syntax::{parse_file, Item, Stmt};

    fn run_src(src: &str) -> String {
        let (_f, prog) = parse_file("t.lhs", src.into()).unwrap();
        let mut buf = Vec::new();
        run_program(&prog, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn hello() {
        let out = run_src(r#"fn main() { print("hello, LHS") }"#);
        assert_eq!(out, "hello, LHS\n");
    }

    #[test]
    fn locals() {
        let out = run_src(
            r#"
fn main() {
    let name = "world"
    let n: i32 = 42
    print(name)
    print(n)
}
"#,
        );
        assert_eq!(out, "world\n42\n");
    }

    #[test]
    fn point_dist() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/06_struct_method.lhs"
        ))
        .unwrap();
        let (_f, prog) = parse_file("06.lhs", src).unwrap();
        for item in &prog.items {
            if let Item::Fn(f) = item {
                if f.name == "dist" {
                    assert_eq!(f.body.stmts.len(), 3, "stmts={:?}", f.body.stmts);
                    match &f.body.stmts[1] {
                        Stmt::Let { name, .. } => assert_eq!(name, "dy"),
                        other => panic!("expected let dy, got {other:?}"),
                    }
                }
            }
        }
        let mut buf = Vec::new();
        run_program(&prog, &mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap().trim(), "5");
    }
}
