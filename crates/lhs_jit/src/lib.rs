//! Cranelift backend for an expanded LHS subset.
//!
//! - `lhsc run --jit` — in-process JIT
//! - `lhsc build --emit=cranelift` — object file + link (true AOT)
//!
//! Subset: numeric/`f64`/strings, Option/Result/custom ADTs, structs + methods,
//! match/`is`/`unwrap`, stdlib, file I/O, `extern "C"` (e.g. `puts`), parallel
//! `task`/`await` via host spawn/join. Bump heap lives in `lhs_mem`.

use cranelift::codegen::ir::UserFuncName;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use np_syntax::{BinOp, Block, Expr, FnItem, Item, Pat, Program, Stmt, TypeBody, TypeRef};
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::mem;
use std::process::Command;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::thread;

const BUILTINS: &[&str] = &[
    "print",
    "abs",
    "min",
    "max",
    "assert",
    "Some",
    "Ok",
    "Err",
    "read_file",
    "write_file",
    "getenv",
    "argc",
    "arg",
    "exit",
    "sleep_ms",
    "now_ms",
    "eprint",
    "gc",
];
const BUILTIN_METHODS: &[&str] = &["len", "unwrap", "sqrt"];

const TAG_NONE: i64 = 0;
const TAG_SOME: i64 = 1;
const TAG_OK: i64 = 2;
const TAG_ERR: i64 = 3;

#[derive(Debug)]
pub struct JitError {
    pub message: String,
}

fn err(msg: impl Into<String>) -> JitError {
    JitError {
        message: msg.into(),
    }
}

/* ---- bump heap via shared lhs_mem (D2) ---- */

#[repr(C)]
struct Cell {
    tag: i64,
    payload: i64,
    extra: i64,
}

fn heap_reset() {
    lhs_mem::reset();
}

unsafe fn heap_alloc(n: usize) -> *mut u8 {
    // Cells and dynamic strings are RC-backed so they can outlive a bump epoch.
    unsafe { lhs_mem::rc_alloc(n) }
}

unsafe fn heap_cstr(s: &str) -> i64 {
    unsafe { lhs_mem::rc_cstr(s) }
}

/* ---- parallel task registry ---- */

static TASK_NEXT: AtomicI64 = AtomicI64::new(1);
static TASKS: LazyLock<Mutex<HashMap<i64, thread::JoinHandle<i64>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

unsafe extern "C" fn host_spawn(fptr: i64) -> i64 {
    let f: extern "C" fn() -> i64 = unsafe { mem::transmute(fptr) };
    let id = TASK_NEXT.fetch_add(1, Ordering::Relaxed);
    let handle = thread::spawn(move || f());
    TASKS.lock().unwrap().insert(id, handle);
    id
}

unsafe extern "C" fn host_join(id: i64) -> i64 {
    let handle = TASKS
        .lock()
        .unwrap()
        .remove(&id)
        .expect("lhs: join unknown task");
    handle.join().unwrap_or(0)
}

unsafe extern "C" fn host_print_i64(n: i64) {
    println!("{n}");
}

unsafe extern "C" fn host_print_str(p: *const i8) {
    if p.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) };
    println!("{}", s.to_string_lossy());
}

unsafe extern "C" fn host_abort() {
    eprintln!("assertion failed");
    std::process::abort();
}

unsafe extern "C" fn host_make_none() -> i64 {
    let p = unsafe { heap_alloc(std::mem::size_of::<Cell>()) as *mut Cell };
    unsafe {
        (*p).tag = TAG_NONE;
        (*p).payload = 0;
        (*p).extra = 0;
        p as i64
    }
}

unsafe extern "C" fn host_make_some(v: i64) -> i64 {
    let p = unsafe { heap_alloc(std::mem::size_of::<Cell>()) as *mut Cell };
    unsafe {
        (*p).tag = TAG_SOME;
        (*p).payload = v;
        (*p).extra = 0;
        p as i64
    }
}

unsafe extern "C" fn host_make_ok(v: i64) -> i64 {
    let p = unsafe { heap_alloc(std::mem::size_of::<Cell>()) as *mut Cell };
    unsafe {
        (*p).tag = TAG_OK;
        (*p).payload = v;
        (*p).extra = 0;
        p as i64
    }
}

unsafe extern "C" fn host_make_err(v: i64) -> i64 {
    let p = unsafe { heap_alloc(std::mem::size_of::<Cell>()) as *mut Cell };
    unsafe {
        (*p).tag = TAG_ERR;
        (*p).payload = v;
        (*p).extra = 0;
        p as i64
    }
}

unsafe extern "C" fn host_make_adt(tag: i64, a: i64, b: i64) -> i64 {
    let p = unsafe { heap_alloc(std::mem::size_of::<Cell>()) as *mut Cell };
    unsafe {
        (*p).tag = tag;
        (*p).payload = a;
        (*p).extra = b;
        p as i64
    }
}

unsafe extern "C" fn host_cell_tag(p: i64) -> i64 {
    if p == 0 {
        return TAG_NONE;
    }
    unsafe { (*(p as *const Cell)).tag }
}

unsafe extern "C" fn host_cell_payload(p: i64) -> i64 {
    if p == 0 {
        return 0;
    }
    unsafe { (*(p as *const Cell)).payload }
}

unsafe extern "C" fn host_cell_extra(p: i64) -> i64 {
    if p == 0 {
        return 0;
    }
    unsafe { (*(p as *const Cell)).extra }
}

unsafe extern "C" fn host_print_f64(bits: i64) {
    println!("{}", f64::from_bits(bits as u64));
}

unsafe extern "C" fn host_fadd(a: i64, b: i64) -> i64 {
    let x = f64::from_bits(a as u64);
    let y = f64::from_bits(b as u64);
    (x + y).to_bits() as i64
}
unsafe extern "C" fn host_fsub(a: i64, b: i64) -> i64 {
    (f64::from_bits(a as u64) - f64::from_bits(b as u64)).to_bits() as i64
}
unsafe extern "C" fn host_fmul(a: i64, b: i64) -> i64 {
    (f64::from_bits(a as u64) * f64::from_bits(b as u64)).to_bits() as i64
}
unsafe extern "C" fn host_fdiv(a: i64, b: i64) -> i64 {
    (f64::from_bits(a as u64) / f64::from_bits(b as u64)).to_bits() as i64
}

unsafe extern "C" fn host_fsqrt(a: i64) -> i64 {
    f64::from_bits(a as u64).sqrt().to_bits() as i64
}

unsafe extern "C" fn host_read_file(path: i64) -> i64 {
    if path == 0 {
        return unsafe { host_make_err(heap_cstr("null path")) };
    }
    let cpath = unsafe { CStr::from_ptr(path as *const i8) };
    let path_str = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return unsafe { host_make_err(heap_cstr("invalid path utf8")) },
    };
    match std::fs::read_to_string(path_str) {
        Ok(s) => unsafe { host_make_ok(heap_cstr(&s)) },
        Err(e) => unsafe { host_make_err(heap_cstr(&e.to_string())) },
    }
}

unsafe extern "C" fn host_write_file(path: i64, contents: i64) -> i64 {
    if path == 0 {
        return unsafe { host_make_err(heap_cstr("null path")) };
    }
    let cpath = unsafe { CStr::from_ptr(path as *const i8) };
    let path_str = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return unsafe { host_make_err(heap_cstr("invalid path utf8")) },
    };
    let body = if contents == 0 {
        ""
    } else {
        match unsafe { CStr::from_ptr(contents as *const i8) }.to_str() {
            Ok(s) => s,
            Err(_) => return unsafe { host_make_err(heap_cstr("invalid contents utf8")) },
        }
    };
    match std::fs::write(path_str, body) {
        Ok(()) => unsafe { host_make_ok(0) },
        Err(e) => unsafe { host_make_err(heap_cstr(&e.to_string())) },
    }
}

unsafe extern "C" fn host_getenv(key: i64) -> i64 {
    if key == 0 {
        return unsafe { host_make_none() };
    }
    let ckey = unsafe { CStr::from_ptr(key as *const i8) };
    let Ok(k) = ckey.to_str() else {
        return unsafe { host_make_none() };
    };
    match std::env::var(k) {
        Ok(v) => unsafe { host_make_some(heap_cstr(&v)) },
        Err(_) => unsafe { host_make_none() },
    }
}

unsafe extern "C" fn host_argc() -> i64 {
    std::env::args().len() as i64
}

unsafe extern "C" fn host_arg(i: i64) -> i64 {
    match std::env::args().nth(i as usize) {
        Some(s) => unsafe { host_make_some(heap_cstr(&s)) },
        None => unsafe { host_make_none() },
    }
}

unsafe extern "C" fn host_exit(code: i64) -> i64 {
    std::process::exit(code as i32);
}

unsafe extern "C" fn host_sleep_ms(ms: i64) -> i64 {
    if ms > 0 {
        thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    0
}

unsafe extern "C" fn host_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

unsafe extern "C" fn host_eprint(p: i64) -> i64 {
    if p != 0 {
        let s = unsafe { CStr::from_ptr(p as *const i8) };
        eprint!("{}", s.to_string_lossy());
    }
    eprintln!();
    0
}

unsafe extern "C" fn host_gc() -> i64 {
    unsafe { lhs_mem::lhs_gc() as i64 }
}

unsafe extern "C" fn host_str_len(p: i64) -> i64 {
    if p == 0 {
        return 0;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p as *const i8) };
    s.to_bytes().len() as i64
}

unsafe extern "C" fn host_str_index(p: i64, i: i64) -> i64 {
    if p == 0 || i < 0 {
        return 0;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p as *const i8) };
    let b = s.to_bytes();
    if (i as usize) >= b.len() {
        return 0;
    }
    b[i as usize] as i64
}

unsafe extern "C" fn host_str_eq(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return i64::from(a == b);
    }
    let sa = unsafe { std::ffi::CStr::from_ptr(a as *const i8) };
    let sb = unsafe { std::ffi::CStr::from_ptr(b as *const i8) };
    i64::from(sa.to_bytes() == sb.to_bytes())
}

fn pat_ok(p: &Pat) -> bool {
    match p {
        Pat::Ident { .. } => true,
        Pat::Call { name, args, .. } => {
            matches!(name.as_str(), "Some" | "Ok" | "Err") && args.iter().all(pat_ok)
        }
        Pat::Struct { fields, .. } => fields.iter().all(|(_, fp)| pat_ok(fp)),
    }
}

fn expr_ok(
    e: &Expr,
    user_fns: &HashSet<&str>,
    user_methods: &HashSet<String>,
    extern_fns: &HashSet<&str>,
) -> bool {
    match e {
        Expr::Ident { .. }
        | Expr::Int { .. }
        | Expr::Float { .. }
        | Expr::Str { .. }
        | Expr::Char { .. }
        | Expr::CStr { .. } => true,
        Expr::Binary { lhs, rhs, .. } => {
            expr_ok(lhs, user_fns, user_methods, extern_fns)
                && expr_ok(rhs, user_fns, user_methods, extern_fns)
        }
        Expr::Group { inner, .. } | Expr::Cast { expr: inner, .. } | Expr::Await { inner, .. } => {
            expr_ok(inner, user_fns, user_methods, extern_fns)
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            expr_ok(cond, user_fns, user_methods, extern_fns)
                && then_block
                    .stmts
                    .iter()
                    .all(|s| stmt_ok(s, user_fns, user_methods, extern_fns))
                && else_block
                    .as_ref()
                    .map(|b| {
                        b.stmts
                            .iter()
                            .all(|s| stmt_ok(s, user_fns, user_methods, extern_fns))
                    })
                    .unwrap_or(true)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            expr_ok(scrutinee, user_fns, user_methods, extern_fns)
                && arms.iter().all(|a| {
                    pat_ok(&a.pat) && expr_ok(&a.body, user_fns, user_methods, extern_fns)
                })
        }
        Expr::Index { base, index, .. } => {
            expr_ok(base, user_fns, user_methods, extern_fns)
                && expr_ok(index, user_fns, user_methods, extern_fns)
        }
        Expr::Is { expr, .. } => expr_ok(expr, user_fns, user_methods, extern_fns),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .all(|(_, e)| expr_ok(e, user_fns, user_methods, extern_fns)),
        Expr::Call { callee, args, .. } => {
            let callee_ok = match callee.as_ref() {
                Expr::Ident { name, .. } => {
                    BUILTINS.contains(&name.as_str())
                        || user_fns.contains(name.as_str())
                        || extern_fns.contains(name.as_str())
                }
                Expr::Field { name, base, .. } => {
                    expr_ok(base, user_fns, user_methods, extern_fns)
                        && (BUILTIN_METHODS.contains(&name.as_str())
                            || user_methods.iter().any(|m| m.ends_with(&format!(".{name}"))))
                }
                _ => false,
            };
            callee_ok
                && args
                    .iter()
                    .all(|a| expr_ok(a, user_fns, user_methods, extern_fns))
        }
        Expr::Field { base, .. } => expr_ok(base, user_fns, user_methods, extern_fns),
        Expr::Task { body, .. } | Expr::Unsafe { body, .. } => body
            .stmts
            .iter()
            .all(|s| stmt_ok(s, user_fns, user_methods, extern_fns)),
        _ => false,
    }
}

fn stmt_ok(
    s: &Stmt,
    user_fns: &HashSet<&str>,
    user_methods: &HashSet<String>,
    extern_fns: &HashSet<&str>,
) -> bool {
    match s {
        Stmt::Let { init, .. } => expr_ok(init, user_fns, user_methods, extern_fns),
        Stmt::Expr(e) => expr_ok(e, user_fns, user_methods, extern_fns),
        Stmt::Return { value, .. } => value
            .as_ref()
            .map(|e| expr_ok(e, user_fns, user_methods, extern_fns))
            .unwrap_or(true),
    }
}

/// Whether the program fits the Cranelift subset.
pub fn is_jit_supported(program: &Program) -> bool {
    let user_fns: HashSet<&str> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Fn(f) if f.receiver.is_none() => Some(f.name.as_str()),
            _ => None,
        })
        .collect();
    let user_methods: HashSet<String> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Fn(f) => f.receiver.as_ref().map(|r| format!("{r}.{}", f.name)),
            _ => None,
        })
        .collect();
    let extern_fns: HashSet<&str> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Extern(b) if b.abi == "C" => Some(b),
            _ => None,
        })
        .flat_map(|b| b.items.iter().map(|f| f.name.as_str()))
        .collect();
    for item in &program.items {
        match item {
            Item::Type(_) | Item::Use(_) => {}
            Item::Extern(b) => {
                if b.abi != "C" {
                    return false;
                }
            }
            Item::Fn(f) => {
                for stmt in &f.body.stmts {
                    if !stmt_ok(stmt, &user_fns, &user_methods, &extern_fns) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn make_isa(is_pic: bool) -> Result<std::sync::Arc<dyn cranelift::codegen::isa::TargetIsa>, JitError> {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder
        .set("is_pic", if is_pic { "true" } else { "false" })
        .unwrap();
    let isa_builder = cranelift_native::builder().map_err(|m| err(m))?;
    isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| err(e.to_string()))
}

struct HostFns {
    print_i: FuncId,
    print_s: FuncId,
    print_f: FuncId,
    abort: FuncId,
    make_none: FuncId,
    make_some: FuncId,
    make_ok: FuncId,
    make_err: FuncId,
    make_adt: FuncId,
    cell_tag: FuncId,
    cell_payload: FuncId,
    cell_extra: FuncId,
    str_len: FuncId,
    str_index: FuncId,
    str_eq: FuncId,
    fadd: FuncId,
    fsub: FuncId,
    fmul: FuncId,
    fdiv: FuncId,
    fsqrt: FuncId,
    read_file: FuncId,
    write_file: FuncId,
    spawn: FuncId,
    join: FuncId,
    getenv: FuncId,
    argc: FuncId,
    arg: FuncId,
    exit: FuncId,
    sleep_ms: FuncId,
    now_ms: FuncId,
    eprint: FuncId,
    gc: FuncId,
}

fn decl_i64_ret0<M: Module>(module: &mut M, name: &str) -> Result<FuncId, JitError> {
    let sig = module.make_signature();
    module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| err(e.to_string()))
}

fn decl_i64_ret1<M: Module>(module: &mut M, name: &str) -> Result<FuncId, JitError> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| err(e.to_string()))
}

fn decl_i64_ret2<M: Module>(module: &mut M, name: &str) -> Result<FuncId, JitError> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| err(e.to_string()))
}

fn decl_i64_ret3<M: Module>(module: &mut M, name: &str) -> Result<FuncId, JitError> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    module
        .declare_function(name, Linkage::Import, &sig)
        .map_err(|e| err(e.to_string()))
}

fn declare_hosts<M: Module>(module: &mut M) -> Result<HostFns, JitError> {
    let mut sig_pi = module.make_signature();
    sig_pi.params.push(AbiParam::new(types::I64));
    let print_i = module
        .declare_function("lhs_jit_print_i64", Linkage::Import, &sig_pi)
        .map_err(|e| err(e.to_string()))?;

    let mut sig_ps = module.make_signature();
    sig_ps.params.push(AbiParam::new(types::I64));
    let print_s = module
        .declare_function("lhs_jit_print_str", Linkage::Import, &sig_ps)
        .map_err(|e| err(e.to_string()))?;

    let mut sig_pf = module.make_signature();
    sig_pf.params.push(AbiParam::new(types::I64));
    let print_f = module
        .declare_function("lhs_jit_print_f64", Linkage::Import, &sig_pf)
        .map_err(|e| err(e.to_string()))?;

    let abort = decl_i64_ret0(module, "lhs_jit_abort")?;

    let mut sig_none = module.make_signature();
    sig_none.returns.push(AbiParam::new(types::I64));
    let make_none = module
        .declare_function("lhs_make_none", Linkage::Import, &sig_none)
        .map_err(|e| err(e.to_string()))?;

    Ok(HostFns {
        print_i,
        print_s,
        print_f,
        abort,
        make_none,
        make_some: decl_i64_ret1(module, "lhs_make_some")?,
        make_ok: decl_i64_ret1(module, "lhs_make_ok")?,
        make_err: decl_i64_ret1(module, "lhs_make_err")?,
        make_adt: decl_i64_ret3(module, "lhs_make_adt")?,
        cell_tag: decl_i64_ret1(module, "lhs_cell_tag")?,
        cell_payload: decl_i64_ret1(module, "lhs_cell_payload")?,
        cell_extra: decl_i64_ret1(module, "lhs_cell_extra")?,
        str_len: decl_i64_ret1(module, "lhs_str_len")?,
        str_index: decl_i64_ret2(module, "lhs_str_index")?,
        str_eq: decl_i64_ret2(module, "lhs_str_eq")?,
        fadd: decl_i64_ret2(module, "lhs_fadd")?,
        fsub: decl_i64_ret2(module, "lhs_fsub")?,
        fmul: decl_i64_ret2(module, "lhs_fmul")?,
        fdiv: decl_i64_ret2(module, "lhs_fdiv")?,
        fsqrt: decl_i64_ret1(module, "lhs_fsqrt")?,
        read_file: decl_i64_ret1(module, "lhs_read_file")?,
        write_file: decl_i64_ret2(module, "lhs_write_file")?,
        spawn: decl_i64_ret1(module, "lhs_spawn")?,
        join: decl_i64_ret1(module, "lhs_join")?,
        getenv: decl_i64_ret1(module, "lhs_getenv")?,
        argc: {
            let mut sig = module.make_signature();
            sig.returns.push(AbiParam::new(types::I64));
            module
                .declare_function("lhs_argc", Linkage::Import, &sig)
                .map_err(|e| err(e.to_string()))?
        },
        arg: decl_i64_ret1(module, "lhs_arg")?,
        exit: decl_i64_ret1(module, "lhs_exit")?,
        sleep_ms: decl_i64_ret1(module, "lhs_sleep_ms")?,
        now_ms: {
            let mut sig = module.make_signature();
            sig.returns.push(AbiParam::new(types::I64));
            module
                .declare_function("lhs_now_ms", Linkage::Import, &sig)
                .map_err(|e| err(e.to_string()))?
        },
        eprint: decl_i64_ret1(module, "lhs_eprint")?,
        gc: {
            let mut sig = module.make_signature();
            sig.returns.push(AbiParam::new(types::I64));
            module
                .declare_function("lhs_gc", Linkage::Import, &sig)
                .map_err(|e| err(e.to_string()))?
        },
    })
}

#[derive(Clone)]
struct VariantInfo {
    tag: i64,
    fields: Vec<String>,
    /// field name -> is f64
    field_float: HashMap<String, bool>,
}

fn collect_variants(program: &Program) -> HashMap<String, VariantInfo> {
    let mut out = HashMap::new();
    let mut next_tag = 10i64;
    for item in &program.items {
        let Item::Type(ti) = item else { continue };
        match &ti.kind {
            TypeBody::Adt(variants) => {
                for v in variants {
                    let mut field_float = HashMap::new();
                    let mut fields = Vec::new();
                    for f in &v.fields {
                        fields.push(f.name.clone());
                        let is_f = matches!(&f.ty, TypeRef::Named { name, .. } if name == "f64");
                        field_float.insert(f.name.clone(), is_f);
                    }
                    out.insert(
                        v.name.clone(),
                        VariantInfo {
                            tag: next_tag,
                            fields,
                            field_float,
                        },
                    );
                    next_tag += 1;
                }
            }
            TypeBody::Struct(sfields) => {
                let mut field_float = HashMap::new();
                let mut fields = Vec::new();
                for f in sfields {
                    fields.push(f.name.clone());
                    let is_f = matches!(&f.ty, TypeRef::Named { name, .. } if name == "f64");
                    field_float.insert(f.name.clone(), is_f);
                }
                out.insert(
                    ti.name.clone(),
                    VariantInfo {
                        tag: next_tag,
                        fields,
                        field_float,
                    },
                );
                next_tag += 1;
            }
        }
    }
    out
}

fn fn_key(f: &FnItem) -> String {
    match &f.receiver {
        Some(r) => format!("{r}.{}", f.name),
        None => f.name.clone(),
    }
}

fn fn_symbol(f: &FnItem) -> String {
    match &f.receiver {
        Some(r) => format!("lhs_{r}_{}", f.name),
        None => format!("lhs_{}", f.name),
    }
}

fn param_is_str(p: &np_syntax::Param) -> bool {
    matches!(
        &p.ty,
        Some(TypeRef::Named { name, .. }) if name == "str"
    )
}

fn define_program<M: Module>(
    module: &mut M,
    program: &Program,
    export_main: bool,
) -> Result<HashMap<String, FuncId>, JitError> {
    let hosts = declare_hosts(module)?;
    let variants = collect_variants(program);

    let mut str_data: HashMap<String, cranelift_module::DataId> = HashMap::new();
    collect_strings(program, &mut |s| {
        if str_data.contains_key(s) {
            return;
        }
        let name = format!("str_{}", str_data.len());
        let id = module
            .declare_data(&name, Linkage::Local, false, false)
            .unwrap();
        let mut desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        if !bytes.ends_with(&[0]) {
            bytes.push(0);
        }
        desc.define(bytes.into_boxed_slice());
        module.define_data(id, &desc).unwrap();
        str_data.insert(s.clone(), id);
    });

    let fns: Vec<&FnItem> = program
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Fn(f) => Some(f),
            _ => None,
        })
        .collect();

    let task_bodies = collect_task_bodies(program);

    // Extern "C" imports (e.g. puts)
    let mut extern_ids: HashMap<String, FuncId> = HashMap::new();
    for item in &program.items {
        let Item::Extern(b) = item else { continue };
        if b.abi != "C" {
            continue;
        }
        for ef in &b.items {
            let mut sig = module.make_signature();
            for _ in &ef.params {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64));
            let id = module
                .declare_function(&ef.name, Linkage::Import, &sig)
                .map_err(|e| err(e.to_string()))?;
            extern_ids.insert(ef.name.clone(), id);
        }
    }

    let mut func_ids = HashMap::new();
    let mut fn_returns_str: HashMap<String, bool> = HashMap::new();
    let mut fn_returns_float: HashMap<String, bool> = HashMap::new();
    for f in &fns {
        let key = fn_key(f);
        let mut sig = module.make_signature();
        for _ in &f.params {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        let linkage = if export_main && f.name == "main" && f.receiver.is_none() {
            Linkage::Export
        } else {
            Linkage::Local
        };
        let id = module
            .declare_function(&fn_symbol(f), linkage, &sig)
            .map_err(|e| err(e.to_string()))?;
        func_ids.insert(key.clone(), id);
        let ret_str = matches!(
            &f.ret,
            Some(TypeRef::Named { name, .. }) if name == "str"
        );
        fn_returns_str.insert(key.clone(), ret_str);
        let ret_float = matches!(
            &f.ret,
            Some(TypeRef::Named { name, .. }) if name == "f64"
        );
        fn_returns_float.insert(key, ret_float);
    }

    let mut task_ids: Vec<FuncId> = Vec::new();
    for (i, _) in task_bodies.iter().enumerate() {
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function(&format!("lhs_task_{i}"), Linkage::Local, &sig)
            .map_err(|e| err(e.to_string()))?;
        task_ids.push(id);
    }

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut task_queue: std::collections::VecDeque<FuncId> =
        task_ids.iter().copied().collect();

    for f in &fns {
        let key = fn_key(f);
        let id = *func_ids.get(&key).unwrap();
        let mut sig = module.make_signature();
        for _ in &f.params {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        ctx.func.signature = sig;
        ctx.func.name = UserFuncName::user(0, id.as_u32());

        {
            let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let entry = bcx.create_block();
            bcx.append_block_params_for_function_params(entry);
            bcx.switch_to_block(entry);
            bcx.seal_block(entry);

            let mut vars: HashMap<String, Variable> = HashMap::new();
            let mut str_vars: HashMap<String, bool> = HashMap::new();
            let mut float_vars: HashMap<String, bool> = HashMap::new();
            let mut struct_vars: HashMap<String, String> = HashMap::new();
            let mut next_var = 0u32;
            for (i, p) in f.params.iter().enumerate() {
                let v = Variable::from_u32(next_var);
                next_var += 1;
                bcx.declare_var(v, types::I64);
                let val = bcx.block_params(entry)[i];
                bcx.def_var(v, val);
                vars.insert(p.name.clone(), v);
                str_vars.insert(p.name.clone(), param_is_str(p));
                float_vars.insert(
                    p.name.clone(),
                    matches!(&p.ty, Some(TypeRef::Named { name, .. }) if name == "f64"),
                );
                if i == 0 {
                    if let Some(r) = &f.receiver {
                        struct_vars.insert(p.name.clone(), r.clone());
                    }
                }
                if let Some(TypeRef::Named { name, .. }) = &p.ty {
                    if variants.contains_key(name) {
                        struct_vars.insert(p.name.clone(), name.clone());
                    }
                }
            }

            let mut gen = Gen {
                bcx: &mut bcx,
                module,
                vars: &mut vars,
                str_vars: &mut str_vars,
                float_vars: &mut float_vars,
                struct_vars: &mut struct_vars,
                next_var: &mut next_var,
                func_ids: &func_ids,
                extern_ids: &extern_ids,
                fn_returns_str: &fn_returns_str,
                fn_returns_float: &fn_returns_float,
                variants: &variants,
                str_data: &str_data,
                hosts: &hosts,
                task_queue: &mut task_queue,
            };
            let ret = gen.emit_block_value(&f.body.stmts)?;
            if !ret.1 {
                gen.bcx.ins().return_(&[ret.0]);
            }
            gen.bcx.seal_all_blocks();
            drop(gen);
            bcx.finalize();
        }
        module
            .define_function(id, &mut ctx)
            .map_err(|e| err(e.to_string()))?;
        module.clear_context(&mut ctx);
    }

    // Define task thunks (bodies collected in program order)
    for (i, body) in task_bodies.iter().enumerate() {
        let id = task_ids[i];
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        ctx.func.signature = sig;
        ctx.func.name = UserFuncName::user(0, id.as_u32());
        {
            let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let entry = bcx.create_block();
            bcx.switch_to_block(entry);
            bcx.seal_block(entry);
            let mut vars = HashMap::new();
            let mut str_vars = HashMap::new();
            let mut float_vars = HashMap::new();
            let mut struct_vars = HashMap::new();
            let mut next_var = 0u32;
            let mut gen = Gen {
                bcx: &mut bcx,
                module,
                vars: &mut vars,
                str_vars: &mut str_vars,
                float_vars: &mut float_vars,
                struct_vars: &mut struct_vars,
                next_var: &mut next_var,
                func_ids: &func_ids,
                extern_ids: &extern_ids,
                fn_returns_str: &fn_returns_str,
                fn_returns_float: &fn_returns_float,
                variants: &variants,
                str_data: &str_data,
                hosts: &hosts,
                task_queue: &mut task_queue,
            };
            let ret = gen.emit_block_value(&body.stmts)?;
            if !ret.1 {
                gen.bcx.ins().return_(&[ret.0]);
            }
            gen.bcx.seal_all_blocks();
            drop(gen);
            bcx.finalize();
        }
        module
            .define_function(id, &mut ctx)
            .map_err(|e| err(e.to_string()))?;
        module.clear_context(&mut ctx);
    }

    Ok(func_ids)
}

fn collect_task_bodies(program: &Program) -> Vec<&Block> {
    let mut out = Vec::new();
    for item in &program.items {
        if let Item::Fn(f) = item {
            walk_collect_tasks(&f.body.stmts, &mut out);
        }
    }
    out
}

fn walk_collect_tasks<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Block>) {
    for s in stmts {
        match s {
            Stmt::Let { init, .. } => walk_collect_tasks_expr(init, out),
            Stmt::Expr(e) => walk_collect_tasks_expr(e, out),
            Stmt::Return { value, .. } => {
                if let Some(e) = value {
                    walk_collect_tasks_expr(e, out);
                }
            }
        }
    }
}

fn walk_collect_tasks_expr<'a>(e: &'a Expr, out: &mut Vec<&'a Block>) {
    match e {
        Expr::Task { body, .. } => {
            out.push(body);
            walk_collect_tasks(&body.stmts, out);
        }
        Expr::Unsafe { body, .. } => walk_collect_tasks(&body.stmts, out),
        Expr::Binary { lhs, rhs, .. } => {
            walk_collect_tasks_expr(lhs, out);
            walk_collect_tasks_expr(rhs, out);
        }
        Expr::Group { inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::Await { inner, .. }
        | Expr::Is { expr: inner, .. } => walk_collect_tasks_expr(inner, out),
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            walk_collect_tasks_expr(cond, out);
            walk_collect_tasks(&then_block.stmts, out);
            if let Some(eb) = else_block {
                walk_collect_tasks(&eb.stmts, out);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_collect_tasks_expr(scrutinee, out);
            for a in arms {
                walk_collect_tasks_expr(&a.body, out);
            }
        }
        Expr::Index { base, index, .. } => {
            walk_collect_tasks_expr(base, out);
            walk_collect_tasks_expr(index, out);
        }
        Expr::Call { callee, args, .. } => {
            walk_collect_tasks_expr(callee, out);
            for a in args {
                walk_collect_tasks_expr(a, out);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, e) in fields {
                walk_collect_tasks_expr(e, out);
            }
        }
        Expr::Field { base, .. } => walk_collect_tasks_expr(base, out),
        _ => {}
    }
}

fn register_host_symbols(jit_builder: &mut JITBuilder) {
    jit_builder.symbol("lhs_jit_print_i64", host_print_i64 as *const u8);
    jit_builder.symbol("lhs_jit_print_str", host_print_str as *const u8);
    jit_builder.symbol("lhs_jit_print_f64", host_print_f64 as *const u8);
    jit_builder.symbol("lhs_jit_abort", host_abort as *const u8);
    jit_builder.symbol("lhs_make_none", host_make_none as *const u8);
    jit_builder.symbol("lhs_make_some", host_make_some as *const u8);
    jit_builder.symbol("lhs_make_ok", host_make_ok as *const u8);
    jit_builder.symbol("lhs_make_err", host_make_err as *const u8);
    jit_builder.symbol("lhs_make_adt", host_make_adt as *const u8);
    jit_builder.symbol("lhs_cell_tag", host_cell_tag as *const u8);
    jit_builder.symbol("lhs_cell_payload", host_cell_payload as *const u8);
    jit_builder.symbol("lhs_cell_extra", host_cell_extra as *const u8);
    jit_builder.symbol("lhs_str_len", host_str_len as *const u8);
    jit_builder.symbol("lhs_str_index", host_str_index as *const u8);
    jit_builder.symbol("lhs_str_eq", host_str_eq as *const u8);
    jit_builder.symbol("lhs_fadd", host_fadd as *const u8);
    jit_builder.symbol("lhs_fsub", host_fsub as *const u8);
    jit_builder.symbol("lhs_fmul", host_fmul as *const u8);
    jit_builder.symbol("lhs_fdiv", host_fdiv as *const u8);
    jit_builder.symbol("lhs_fsqrt", host_fsqrt as *const u8);
    jit_builder.symbol("lhs_read_file", host_read_file as *const u8);
    jit_builder.symbol("lhs_write_file", host_write_file as *const u8);
    jit_builder.symbol("lhs_spawn", host_spawn as *const u8);
    jit_builder.symbol("lhs_join", host_join as *const u8);
    jit_builder.symbol("lhs_getenv", host_getenv as *const u8);
    jit_builder.symbol("lhs_argc", host_argc as *const u8);
    jit_builder.symbol("lhs_arg", host_arg as *const u8);
    jit_builder.symbol("lhs_exit", host_exit as *const u8);
    jit_builder.symbol("lhs_sleep_ms", host_sleep_ms as *const u8);
    jit_builder.symbol("lhs_now_ms", host_now_ms as *const u8);
    jit_builder.symbol("lhs_eprint", host_eprint as *const u8);
    jit_builder.symbol("lhs_gc", host_gc as *const u8);
    // libc
    jit_builder.symbol("puts", libc::puts as *const u8);
}

/// JIT-compile and run `main`.
pub fn run_jit(program: &Program) -> Result<(), JitError> {
    if !is_jit_supported(program) {
        return Err(err(
            "Cranelift: unsupported construct. Use plain `lhsc run` or default embed build.",
        ));
    }

    heap_reset();
    {
        let mut t = TASKS.lock().unwrap();
        t.clear();
    }
    let isa = make_isa(false)?;
    let mut jit_builder = JITBuilder::with_isa(isa, default_libcall_names());
    register_host_symbols(&mut jit_builder);
    let mut module = JITModule::new(jit_builder);

    let func_ids = define_program(&mut module, program, false)?;
    module
        .finalize_definitions()
        .map_err(|e| err(e.to_string()))?;

    let main_id = *func_ids.get("main").ok_or_else(|| err("no fn main"))?;
    let code = module.get_finalized_function(main_id);
    let main_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(code) };
    let _ = main_fn();
    Ok(())
}

const STUBS_C: &str = r#"/* Generated by lhsc --emit=cranelift — RC cells + hosts */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <errno.h>
#include <pthread.h>
#include <time.h>
#include <unistd.h>

enum { TAG_NONE = 0, TAG_SOME = 1, TAG_OK = 2, TAG_ERR = 3 };

typedef struct { int64_t rc; uint64_t size; } LhsRcHdr;
typedef struct { int64_t tag; int64_t payload; int64_t extra; } LhsCell;

static void *lhs_rc_alloc(size_t n) {
    LhsRcHdr *h = (LhsRcHdr *)calloc(1, sizeof(LhsRcHdr) + n);
    if (!h) abort();
    h->rc = 1;
    h->size = (uint64_t)n;
    return (char *)h + sizeof(LhsRcHdr);
}

static int64_t lhs_heap_cstr(const char *s) {
    size_t n = s ? strlen(s) : 0;
    char *p = (char *)lhs_rc_alloc(n + 1);
    if (s) memcpy(p, s, n);
    p[n] = 0;
    return (int64_t)(uintptr_t)p;
}

int64_t lhs_make_none(void) {
    LhsCell *c = (LhsCell *)lhs_rc_alloc(sizeof(LhsCell));
    c->tag = TAG_NONE; c->payload = 0; c->extra = 0;
    return (int64_t)(uintptr_t)c;
}
int64_t lhs_make_some(int64_t v) {
    LhsCell *c = (LhsCell *)lhs_rc_alloc(sizeof(LhsCell));
    c->tag = TAG_SOME; c->payload = v; c->extra = 0;
    return (int64_t)(uintptr_t)c;
}
int64_t lhs_make_ok(int64_t v) {
    LhsCell *c = (LhsCell *)lhs_rc_alloc(sizeof(LhsCell));
    c->tag = TAG_OK; c->payload = v; c->extra = 0;
    return (int64_t)(uintptr_t)c;
}
int64_t lhs_make_err(int64_t v) {
    LhsCell *c = (LhsCell *)lhs_rc_alloc(sizeof(LhsCell));
    c->tag = TAG_ERR; c->payload = v; c->extra = 0;
    return (int64_t)(uintptr_t)c;
}
int64_t lhs_make_adt(int64_t tag, int64_t a, int64_t b) {
    LhsCell *c = (LhsCell *)lhs_rc_alloc(sizeof(LhsCell));
    c->tag = tag; c->payload = a; c->extra = b;
    return (int64_t)(uintptr_t)c;
}
int64_t lhs_cell_tag(int64_t p) {
    if (!p) return TAG_NONE;
    return ((LhsCell *)(uintptr_t)p)->tag;
}
int64_t lhs_cell_payload(int64_t p) {
    if (!p) return 0;
    return ((LhsCell *)(uintptr_t)p)->payload;
}
int64_t lhs_cell_extra(int64_t p) {
    if (!p) return 0;
    return ((LhsCell *)(uintptr_t)p)->extra;
}
int64_t lhs_str_len(int64_t p) {
    if (!p) return 0;
    return (int64_t)strlen((const char *)(uintptr_t)p);
}
int64_t lhs_str_index(int64_t p, int64_t i) {
    if (!p || i < 0) return 0;
    const unsigned char *s = (const unsigned char *)(uintptr_t)p;
    size_t n = strlen((const char *)s);
    if ((size_t)i >= n) return 0;
    return (int64_t)s[i];
}
int64_t lhs_str_eq(int64_t a, int64_t b) {
    if (!a || !b) return a == b;
    return strcmp((const char *)(uintptr_t)a, (const char *)(uintptr_t)b) == 0;
}
static double bits_to_f(int64_t b) { double d; memcpy(&d, &b, 8); return d; }
static int64_t f_to_bits(double d) { int64_t b; memcpy(&b, &d, 8); return b; }
int64_t lhs_fadd(int64_t a, int64_t b) { return f_to_bits(bits_to_f(a) + bits_to_f(b)); }
int64_t lhs_fsub(int64_t a, int64_t b) { return f_to_bits(bits_to_f(a) - bits_to_f(b)); }
int64_t lhs_fmul(int64_t a, int64_t b) { return f_to_bits(bits_to_f(a) * bits_to_f(b)); }
int64_t lhs_fdiv(int64_t a, int64_t b) { return f_to_bits(bits_to_f(a) / bits_to_f(b)); }
int64_t lhs_fsqrt(int64_t a) { return f_to_bits(sqrt(bits_to_f(a))); }

/* parallel tasks via pthreads */
typedef struct { int64_t (*fn)(void); int64_t result; } LhsTaskArg;
static pthread_mutex_t lhs_task_mu = PTHREAD_MUTEX_INITIALIZER;
static int64_t lhs_task_next = 1;
enum { LHS_TASK_MAX = 256 };
static struct { int used; pthread_t th; int64_t result; } lhs_task_tab[LHS_TASK_MAX];

static void *lhs_task_entry(void *p) {
    LhsTaskArg *a = (LhsTaskArg *)p;
    a->result = a->fn();
    return a;
}

int64_t lhs_spawn(int64_t fptr) {
    pthread_mutex_lock(&lhs_task_mu);
    int64_t id = lhs_task_next++;
    int slot = -1;
    for (int i = 0; i < LHS_TASK_MAX; i++) {
        if (!lhs_task_tab[i].used) { slot = i; break; }
    }
    if (slot < 0) { pthread_mutex_unlock(&lhs_task_mu); abort(); }
    lhs_task_tab[slot].used = 1;
    pthread_mutex_unlock(&lhs_task_mu);
    LhsTaskArg *arg = (LhsTaskArg *)malloc(sizeof(LhsTaskArg));
    arg->fn = (int64_t(*)(void))(uintptr_t)fptr;
    arg->result = 0;
    if (pthread_create(&lhs_task_tab[slot].th, NULL, lhs_task_entry, arg) != 0) abort();
    lhs_task_tab[slot].result = (int64_t)(uintptr_t)arg;
    return id * 1000 + slot;
}

int64_t lhs_join(int64_t handle) {
    int slot = (int)(handle % 1000);
    if (slot < 0 || slot >= LHS_TASK_MAX) abort();
    void *ret = NULL;
    pthread_join(lhs_task_tab[slot].th, &ret);
    LhsTaskArg *arg = (LhsTaskArg *)ret;
    int64_t r = arg ? arg->result : 0;
    free(arg);
    pthread_mutex_lock(&lhs_task_mu);
    lhs_task_tab[slot].used = 0;
    pthread_mutex_unlock(&lhs_task_mu);
    return r;
}

int64_t lhs_read_file(int64_t path) {
    if (!path) return lhs_make_err(lhs_heap_cstr("null path"));
    FILE *f = fopen((const char *)(uintptr_t)path, "rb");
    if (!f) return lhs_make_err(lhs_heap_cstr(strerror(errno)));
    if (fseek(f, 0, SEEK_END) != 0) { fclose(f); return lhs_make_err(lhs_heap_cstr(strerror(errno))); }
    long sz = ftell(f);
    if (sz < 0) { fclose(f); return lhs_make_err(lhs_heap_cstr(strerror(errno))); }
    rewind(f);
    char *buf = (char *)lhs_rc_alloc((size_t)sz + 1);
    size_t n = fread(buf, 1, (size_t)sz, f);
    buf[n] = 0;
    fclose(f);
    return lhs_make_ok((int64_t)(uintptr_t)buf);
}

int64_t lhs_write_file(int64_t path, int64_t contents) {
    if (!path) return lhs_make_err(lhs_heap_cstr("null path"));
    FILE *f = fopen((const char *)(uintptr_t)path, "wb");
    if (!f) return lhs_make_err(lhs_heap_cstr(strerror(errno)));
    const char *s = contents ? (const char *)(uintptr_t)contents : "";
    size_t n = strlen(s);
    if (fwrite(s, 1, n, f) != n) { fclose(f); return lhs_make_err(lhs_heap_cstr(strerror(errno))); }
    fclose(f);
    return lhs_make_ok(0);
}

int64_t lhs_getenv(int64_t key) {
    if (!key) return lhs_make_none();
    const char *v = getenv((const char *)(uintptr_t)key);
    if (!v) return lhs_make_none();
    return lhs_make_some(lhs_heap_cstr(v));
}
int64_t lhs_argc(void) { return (int64_t)1; /* AOT binaries: stub */ }
int64_t lhs_arg(int64_t i) {
    (void)i;
    return lhs_make_none();
}
void lhs_exit(int64_t code) { _exit((int)code); }
void lhs_sleep_ms(int64_t ms) {
    if (ms > 0) {
        struct timespec ts;
        ts.tv_sec = ms / 1000;
        ts.tv_nsec = (ms % 1000) * 1000000L;
        nanosleep(&ts, NULL);
    }
}
int64_t lhs_now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}
void lhs_eprint(int64_t p) {
    if (p) fputs((const char *)(uintptr_t)p, stderr);
    fputc('\n', stderr);
}
int64_t lhs_gc(void) { return 0; }

void lhs_jit_print_i64(int64_t n) { printf("%lld\n", (long long)n); }
void lhs_jit_print_str(const char *p) { if (p) puts(p); }
void lhs_jit_print_f64(int64_t bits) { printf("%.17g\n", bits_to_f(bits)); }
void lhs_jit_abort(void) { fprintf(stderr, "assertion failed\n"); abort(); }

extern int64_t lhs_main(void);
int main(void) { lhs_main(); return 0; }
"#;



/// Compile subset program to a native binary via Cranelift object + system linker.
pub fn build_aot(program: &Program, out_path: &str) -> Result<(), JitError> {
    if !is_jit_supported(program) {
        return Err(err(
            "--emit=cranelift: unsupported construct (use default embed build)",
        ));
    }

    let isa = make_isa(true)?;
    let builder = ObjectBuilder::new(isa, "lhs", default_libcall_names())
        .map_err(|e| err(e.to_string()))?;
    let mut module = ObjectModule::new(builder);

    let _ = define_program(&mut module, program, true)?;
    let product = module.finish();
    let bytes = product.emit().map_err(|e| err(e.to_string()))?;

    let obj_path = format!("{out_path}.lhs.o");
    let stubs_path = format!("{out_path}.lhs.stubs.c");
    std::fs::write(&obj_path, &bytes).map_err(|e| err(e.to_string()))?;
    std::fs::write(&stubs_path, STUBS_C).map_err(|e| err(e.to_string()))?;

    let status = Command::new("cc")
        .args([&obj_path, &stubs_path, "-lm", "-lpthread", "-ldl", "-o", out_path])
        .status()
        .map_err(|e| err(format!("failed to run cc: {e}")))?;
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(&stubs_path);
    if !status.success() {
        return Err(err("cc link failed for Cranelift AOT binary"));
    }
    // Also ensure lhs_mem is built so embed/JIT share the same RC crate.
    let _ = Command::new("cargo")
        .args(["build", "-p", "lhs_mem", "-q"])
        .status();
    Ok(())
}

fn collect_strings(program: &Program, f: &mut dyn FnMut(&String)) {
    for item in &program.items {
        if let Item::Fn(func) = item {
            walk_stmts(&func.body.stmts, f);
        }
    }
}

fn walk_stmts(stmts: &[Stmt], f: &mut dyn FnMut(&String)) {
    for s in stmts {
        match s {
            Stmt::Let { init, .. } => walk_expr(init, f),
            Stmt::Expr(e) => walk_expr(e, f),
            Stmt::Return { value, .. } => {
                if let Some(e) = value {
                    walk_expr(e, f);
                }
            }
        }
    }
}

fn walk_expr(e: &Expr, f: &mut dyn FnMut(&String)) {
    match e {
        Expr::Str { value, .. } | Expr::CStr { value, .. } => f(value),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        Expr::Group { inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::Await { inner, .. }
        | Expr::Is { expr: inner, .. } => walk_expr(inner, f),
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            walk_expr(cond, f);
            walk_stmts(&then_block.stmts, f);
            if let Some(eb) = else_block {
                walk_stmts(&eb.stmts, f);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(scrutinee, f);
            for a in arms {
                walk_expr(&a.body, f);
            }
        }
        Expr::Index { base, index, .. } => {
            walk_expr(base, f);
            walk_expr(index, f);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, e) in fields {
                walk_expr(e, f);
            }
        }
        Expr::Field { base, .. } => walk_expr(base, f),
        Expr::Task { body, .. } | Expr::Unsafe { body, .. } => walk_stmts(&body.stmts, f),
        _ => {}
    }
}

struct Gen<'a, 'b, M: Module> {
    bcx: &'a mut FunctionBuilder<'b>,
    module: &'a mut M,
    vars: &'a mut HashMap<String, Variable>,
    str_vars: &'a mut HashMap<String, bool>,
    float_vars: &'a mut HashMap<String, bool>,
    struct_vars: &'a mut HashMap<String, String>,
    next_var: &'a mut u32,
    func_ids: &'a HashMap<String, FuncId>,
    extern_ids: &'a HashMap<String, FuncId>,
    fn_returns_str: &'a HashMap<String, bool>,
    fn_returns_float: &'a HashMap<String, bool>,
    variants: &'a HashMap<String, VariantInfo>,
    str_data: &'a HashMap<String, cranelift_module::DataId>,
    hosts: &'a HostFns,
    task_queue: &'a mut std::collections::VecDeque<FuncId>,
}

impl<M: Module> Gen<'_, '_, M> {
    fn alloc_var(&mut self) -> Variable {
        let v = Variable::from_u32(*self.next_var);
        *self.next_var += 1;
        self.bcx.declare_var(v, types::I64);
        v
    }

    fn call1(&mut self, fid: FuncId, a: Value) -> Value {
        let fref = self.module.declare_func_in_func(fid, self.bcx.func);
        let call = self.bcx.ins().call(fref, &[a]);
        self.bcx.inst_results(call)[0]
    }

    fn call0(&mut self, fid: FuncId) -> Value {
        let fref = self.module.declare_func_in_func(fid, self.bcx.func);
        let call = self.bcx.ins().call(fref, &[]);
        self.bcx.inst_results(call)[0]
    }

    fn call2(&mut self, fid: FuncId, a: Value, b: Value) -> Value {
        let fref = self.module.declare_func_in_func(fid, self.bcx.func);
        let call = self.bcx.ins().call(fref, &[a, b]);
        self.bcx.inst_results(call)[0]
    }

    fn call3(&mut self, fid: FuncId, a: Value, b: Value, c: Value) -> Value {
        let fref = self.module.declare_func_in_func(fid, self.bcx.func);
        let call = self.bcx.ins().call(fref, &[a, b, c]);
        self.bcx.inst_results(call)[0]
    }

    /// Emit statements; returns (value, block_terminated_with_return).
    fn emit_block_value(&mut self, stmts: &[Stmt]) -> Result<(Value, bool), JitError> {
        let mut last = self.bcx.ins().iconst(types::I64, 0);
        if let Some((end, rest)) = stmts.split_last() {
            for s in rest {
                self.emit_stmt(s)?;
                // mid-block return already filled the block
            }
            match end {
                Stmt::Expr(e) => last = self.emit_expr(e)?,
                Stmt::Return { value, .. } => {
                    let v = match value {
                        Some(e) => self.emit_expr(e)?,
                        None => self.bcx.ins().iconst(types::I64, 0),
                    };
                    self.bcx.ins().return_(&[v]);
                    return Ok((v, true));
                }
                other => {
                    self.emit_stmt(other)?;
                }
            }
        }
        Ok((last, false))
    }

    fn expr_struct_type(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Ident { name, .. } => self.struct_vars.get(name).cloned(),
            Expr::StructLit { name, .. } => Some(name.clone()),
            Expr::Group { inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::Await { inner, .. } => self.expr_struct_type(inner),
            _ => None,
        }
    }

    fn expr_is_str(&self, e: &Expr) -> bool {
        match e {
            Expr::Str { .. } | Expr::CStr { .. } => true,
            Expr::Ident { name, .. } => self.str_vars.get(name).copied().unwrap_or(false),
            Expr::Group { inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::Await { inner, .. } => self.expr_is_str(inner),
            Expr::Call { callee, .. } => {
                if let Expr::Ident { name, .. } = callee.as_ref() {
                    self.fn_returns_str.get(name).copied().unwrap_or(false)
                } else {
                    false
                }
            }
            Expr::Match { arms, .. } => arms.iter().any(|a| self.expr_is_str(&a.body)),
            Expr::If {
                then_block,
                else_block,
                ..
            } => {
                let t = then_block
                    .stmts
                    .last()
                    .map(|s| match s {
                        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => self.expr_is_str(e),
                        _ => false,
                    })
                    .unwrap_or(false);
                let e = else_block
                    .as_ref()
                    .and_then(|b| b.stmts.last())
                    .map(|s| match s {
                        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => self.expr_is_str(e),
                        _ => false,
                    })
                    .unwrap_or(false);
                t || e
            }
            _ => false,
        }
    }

    fn expr_is_float(&self, e: &Expr) -> bool {
        match e {
            Expr::Float { .. } => true,
            Expr::Ident { name, .. } => self.float_vars.get(name).copied().unwrap_or(false),
            Expr::Group { inner, .. }
            | Expr::Cast { expr: inner, .. }
            | Expr::Await { inner, .. } => self.expr_is_float(inner),
            Expr::Binary { lhs, rhs, .. } => self.expr_is_float(lhs) || self.expr_is_float(rhs),
            Expr::Field { base, name, .. } => {
                if let Some(ty) = self.expr_struct_type(base) {
                    if let Some(info) = self.variants.get(&ty) {
                        return info.field_float.get(name).copied().unwrap_or(false);
                    }
                }
                false
            }
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident { name, .. } => {
                    self.fn_returns_float.get(name).copied().unwrap_or(false)
                }
                Expr::Field { name, base, .. } if name == "sqrt" => true,
                Expr::Field { name, base, .. } => {
                    if let Some(ty) = self.expr_struct_type(base) {
                        let key = format!("{ty}.{name}");
                        return self.fn_returns_float.get(&key).copied().unwrap_or(false);
                    }
                    false
                }
                _ => false,
            },
            Expr::Match { arms, .. } => arms.iter().any(|a| self.expr_is_float(&a.body)),
            Expr::If {
                then_block,
                else_block,
                ..
            } => {
                let t = then_block
                    .stmts
                    .last()
                    .map(|s| match s {
                        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => {
                            self.expr_is_float(e)
                        }
                        _ => false,
                    })
                    .unwrap_or(false);
                let e = else_block
                    .as_ref()
                    .and_then(|b| b.stmts.last())
                    .map(|s| match s {
                        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => {
                            self.expr_is_float(e)
                        }
                        _ => false,
                    })
                    .unwrap_or(false);
                t || e
            }
            _ => false,
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<(), JitError> {
        match stmt {
            Stmt::Let { name, init, .. } => {
                let is_str = self.expr_is_str(init);
                let is_float = self.expr_is_float(init);
                let struct_ty = self.expr_struct_type(init);
                let val = self.emit_expr(init)?;
                let v = if let Some(v) = self.vars.get(name) {
                    *v
                } else {
                    let v = self.alloc_var();
                    self.vars.insert(name.clone(), v);
                    v
                };
                self.bcx.def_var(v, val);
                self.str_vars.insert(name.clone(), is_str);
                self.float_vars.insert(name.clone(), is_float);
                if let Some(ty) = struct_ty {
                    self.struct_vars.insert(name.clone(), ty);
                } else {
                    self.struct_vars.remove(name);
                }
            }
            Stmt::Expr(e) => {
                let _ = self.emit_expr(e)?;
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.emit_expr(e)?,
                    None => self.bcx.ins().iconst(types::I64, 0),
                };
                self.bcx.ins().return_(&[v]);
            }
        }
        Ok(())
    }

    fn emit_expr(&mut self, e: &Expr) -> Result<Value, JitError> {
        match e {
            Expr::Int { value, .. } => Ok(self.bcx.ins().iconst(types::I64, *value)),
            Expr::Float { text, .. } => {
                let x: f64 = text.parse().unwrap_or(0.0);
                Ok(self.bcx.ins().iconst(types::I64, x.to_bits() as i64))
            }
            Expr::Char { value, .. } => Ok(self.bcx.ins().iconst(types::I64, *value as i64)),
            Expr::Ident { name, .. } => {
                if name == "None" {
                    return Ok(self.call0(self.hosts.make_none));
                }
                let v = self
                    .vars
                    .get(name)
                    .ok_or_else(|| err(format!("undefined `{name}`")))?;
                Ok(self.bcx.use_var(*v))
            }
            Expr::Str { value, .. } | Expr::CStr { value, .. } => {
                let id = self
                    .str_data
                    .get(value)
                    .ok_or_else(|| err("missing string data"))?;
                let gv = self.module.declare_data_in_func(*id, self.bcx.func);
                Ok(self.bcx.ins().global_value(types::I64, gv))
            }
            Expr::Group { inner, .. }
            | Expr::Cast { expr: inner, .. } => self.emit_expr(inner),
            Expr::Await { inner, .. } => {
                let v = self.emit_expr(inner)?;
                Ok(self.call1(self.hosts.join, v))
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let a = self.emit_expr(lhs)?;
                let b = self.emit_expr(rhs)?;
                if matches!(op, BinOp::Eq | BinOp::Ne)
                    && (self.expr_is_str(lhs) || self.expr_is_str(rhs))
                {
                    let eq = self.call2(self.hosts.str_eq, a, b);
                    return Ok(match op {
                        BinOp::Eq => eq,
                        BinOp::Ne => {
                            let one = self.bcx.ins().iconst(types::I64, 1);
                            self.bcx.ins().isub(one, eq)
                        }
                        _ => unreachable!(),
                    });
                }
                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div)
                    && (self.expr_is_float(lhs) || self.expr_is_float(rhs))
                {
                    let fid = match op {
                        BinOp::Add => self.hosts.fadd,
                        BinOp::Sub => self.hosts.fsub,
                        BinOp::Mul => self.hosts.fmul,
                        BinOp::Div => self.hosts.fdiv,
                        _ => unreachable!(),
                    };
                    return Ok(self.call2(fid, a, b));
                }
                Ok(match op {
                    BinOp::Add => self.bcx.ins().iadd(a, b),
                    BinOp::Sub => self.bcx.ins().isub(a, b),
                    BinOp::Mul => self.bcx.ins().imul(a, b),
                    BinOp::Div => self.bcx.ins().sdiv(a, b),
                    BinOp::Eq => {
                        let c = self.bcx.ins().icmp(IntCC::Equal, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::Ne => {
                        let c = self.bcx.ins().icmp(IntCC::NotEqual, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::Lt => {
                        let c = self.bcx.ins().icmp(IntCC::SignedLessThan, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::Le => {
                        let c = self.bcx.ins().icmp(IntCC::SignedLessThanOrEqual, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::Gt => {
                        let c = self.bcx.ins().icmp(IntCC::SignedGreaterThan, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::Ge => {
                        let c = self.bcx.ins().icmp(IntCC::SignedGreaterThanOrEqual, a, b);
                        self.bcx.ins().uextend(types::I64, c)
                    }
                    BinOp::And => self.bcx.ins().band(a, b),
                    BinOp::Or => self.bcx.ins().bor(a, b),
                })
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                let c = self.emit_expr(cond)?;
                let zero = self.bcx.ins().iconst(types::I64, 0);
                let cond_b = self.bcx.ins().icmp(IntCC::NotEqual, c, zero);
                let then_b = self.bcx.create_block();
                let else_b = self.bcx.create_block();
                let merge = self.bcx.create_block();
                self.bcx.append_block_param(merge, types::I64);
                self.bcx.ins().brif(cond_b, then_b, &[], else_b, &[]);

                self.bcx.switch_to_block(then_b);
                self.bcx.seal_block(then_b);
                let (t, then_ret) = self.emit_block_value(&then_block.stmts)?;
                let mut reached_merge = false;
                if !then_ret {
                    self.bcx.ins().jump(merge, &[t]);
                    reached_merge = true;
                }

                self.bcx.switch_to_block(else_b);
                self.bcx.seal_block(else_b);
                let (ev, else_ret) = if let Some(eb) = else_block {
                    self.emit_block_value(&eb.stmts)?
                } else {
                    (self.bcx.ins().iconst(types::I64, 0), false)
                };
                if !else_ret {
                    self.bcx.ins().jump(merge, &[ev]);
                    reached_merge = true;
                }

                if reached_merge {
                    self.bcx.switch_to_block(merge);
                    self.bcx.seal_block(merge);
                    Ok(self.bcx.block_params(merge)[0])
                } else {
                    self.bcx.seal_block(merge);
                    let cont = self.bcx.create_block();
                    self.bcx.switch_to_block(cont);
                    self.bcx.seal_block(cont);
                    Ok(self.bcx.ins().iconst(types::I64, 0))
                }
            }
            Expr::Index { base, index, .. } => {
                let p = self.emit_expr(base)?;
                let i = self.emit_expr(index)?;
                Ok(self.call2(self.hosts.str_index, p, i))
            }
            Expr::Field { base, name, .. } if name == "len" => {
                let p = self.emit_expr(base)?;
                Ok(self.call1(self.hosts.str_len, p))
            }
            Expr::Field { base, name, .. } => {
                let ty = self.expr_struct_type(base).ok_or_else(|| {
                    err(format!("cannot resolve type for field `{name}`"))
                })?;
                let info = self
                    .variants
                    .get(&ty)
                    .ok_or_else(|| err(format!("unknown type `{ty}`")))?
                    .clone();
                let idx = info
                    .fields
                    .iter()
                    .position(|f| f == name)
                    .ok_or_else(|| err(format!("no field `{name}` on `{ty}`")))?;
                if idx >= 2 {
                    return Err(err("struct fields beyond 2 not yet in Cranelift"));
                }
                let cell = self.emit_expr(base)?;
                Ok(if idx == 0 {
                    self.call1(self.hosts.cell_payload, cell)
                } else {
                    self.call1(self.hosts.cell_extra, cell)
                })
            }
            Expr::Is { expr, pat, .. } => {
                let v = self.emit_expr(expr)?;
                self.emit_pat_test(v, pat)
            }
            Expr::StructLit { name, fields, .. } => self.emit_struct_lit(name, fields),
            Expr::Match {
                scrutinee, arms, ..
            } => self.emit_match(scrutinee, arms),
            Expr::Call { callee, args, .. } => self.emit_call(callee, args),
            Expr::Task { body: _, .. } => {
                let tid = self
                    .task_queue
                    .pop_front()
                    .ok_or_else(|| err("internal: task thunk missing"))?;
                let fref = self.module.declare_func_in_func(tid, self.bcx.func);
                let fptr = self.bcx.ins().func_addr(types::I64, fref);
                Ok(self.call1(self.hosts.spawn, fptr))
            }
            Expr::Unsafe { body, .. } => Ok(self.emit_block_value(&body.stmts)?.0),
            _ => Err(err("unsupported expression in Cranelift subset")),
        }
    }

    fn emit_pat_test(&mut self, scrut: Value, pat: &Pat) -> Result<Value, JitError> {
        match pat {
            Pat::Ident { name, .. } if name == "None" => {
                let tag = self.call1(self.hosts.cell_tag, scrut);
                let want = self.bcx.ins().iconst(types::I64, TAG_NONE);
                let c = self.bcx.ins().icmp(IntCC::Equal, tag, want);
                Ok(self.bcx.ins().uextend(types::I64, c))
            }
            Pat::Ident { name, .. }
                if name.chars().next().is_some_and(|c| c.is_uppercase()) =>
            {
                // nullary ctor name — treat as tag name Some/Ok/Err without args (rare)
                let _ = name;
                Ok(self.bcx.ins().iconst(types::I64, 0))
            }
            Pat::Call { name, .. } => {
                let want_tag = match name.as_str() {
                    "Some" => TAG_SOME,
                    "Ok" => TAG_OK,
                    "Err" => TAG_ERR,
                    _ => return Ok(self.bcx.ins().iconst(types::I64, 0)),
                };
                let tag = self.call1(self.hosts.cell_tag, scrut);
                let want = self.bcx.ins().iconst(types::I64, want_tag);
                let c = self.bcx.ins().icmp(IntCC::Equal, tag, want);
                Ok(self.bcx.ins().uextend(types::I64, c))
            }
            Pat::Struct { name, .. } => {
                let Some(info) = self.variants.get(name) else {
                    return Ok(self.bcx.ins().iconst(types::I64, 0));
                };
                let tag = self.call1(self.hosts.cell_tag, scrut);
                let want = self.bcx.ins().iconst(types::I64, info.tag);
                let c = self.bcx.ins().icmp(IntCC::Equal, tag, want);
                Ok(self.bcx.ins().uextend(types::I64, c))
            }
            _ => Ok(self.bcx.ins().iconst(types::I64, 1)), // wildcard
        }
    }

    fn emit_struct_lit(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
    ) -> Result<Value, JitError> {
        let info = self
            .variants
            .get(name)
            .ok_or_else(|| err(format!("unknown variant `{name}` in Cranelift")))?
            .clone();
        let mut vals = [self.bcx.ins().iconst(types::I64, 0); 2];
        for (i, fname) in info.fields.iter().enumerate() {
            if i >= 2 {
                return Err(err("ADT variants with >2 fields not yet in Cranelift"));
            }
            if let Some((_, e)) = fields.iter().find(|(n, _)| n == fname) {
                vals[i] = self.emit_expr(e)?;
            }
        }
        let tag = self.bcx.ins().iconst(types::I64, info.tag);
        Ok(self.call3(self.hosts.make_adt, tag, vals[0], vals[1]))
    }

    fn emit_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[np_syntax::MatchArm],
    ) -> Result<Value, JitError> {
        let scrut = self.emit_expr(scrutinee)?;
        let merge = self.bcx.create_block();
        self.bcx.append_block_param(merge, types::I64);

        for arm in arms {
            let test = self.emit_pat_test(scrut, &arm.pat)?;
            let zero = self.bcx.ins().iconst(types::I64, 0);
            let cond = self.bcx.ins().icmp(IntCC::NotEqual, test, zero);
            let taken = self.bcx.create_block();
            let next = self.bcx.create_block();
            self.bcx.ins().brif(cond, taken, &[], next, &[]);

            self.bcx.switch_to_block(taken);
            self.bcx.seal_block(taken);
            self.bind_pat(scrut, &arm.pat)?;
            let body = self.emit_expr(&arm.body)?;
            self.bcx.ins().jump(merge, &[body]);

            self.bcx.switch_to_block(next);
            self.bcx.seal_block(next);
        }

        // fallthrough: abort
        let fref = self
            .module
            .declare_func_in_func(self.hosts.abort, self.bcx.func);
        self.bcx.ins().call(fref, &[]);
        let z = self.bcx.ins().iconst(types::I64, 0);
        self.bcx.ins().jump(merge, &[z]);

        self.bcx.switch_to_block(merge);
        self.bcx.seal_block(merge);
        Ok(self.bcx.block_params(merge)[0])
    }

    fn bind_pat(&mut self, scrut: Value, pat: &Pat) -> Result<(), JitError> {
        match pat {
            Pat::Ident { name, .. } => {
                if name == "None" || name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    return Ok(());
                }
                let v = self.alloc_var();
                self.bcx.def_var(v, scrut);
                self.vars.insert(name.clone(), v);
                // inherit unknown — leave str false
                self.str_vars.insert(name.clone(), false);
            }
            Pat::Call { name, args, .. } => {
                let payload = self.call1(self.hosts.cell_payload, scrut);
                let is_str_payload = name == "Err" || name == "Ok" || name == "Some";
                for a in args {
                    match a {
                        Pat::Ident { name: bn, .. }
                            if !bn.chars().next().is_some_and(|c| c.is_uppercase()) =>
                        {
                            let v = self.alloc_var();
                            self.bcx.def_var(v, payload);
                            self.vars.insert(bn.clone(), v);
                            self.str_vars.insert(bn.clone(), is_str_payload);
                            self.float_vars.insert(bn.clone(), false);
                        }
                        other => self.bind_pat(payload, other)?,
                    }
                }
            }
            Pat::Struct { name, fields, .. } => {
                let info = self
                    .variants
                    .get(name)
                    .ok_or_else(|| err(format!("unknown variant `{name}`")))?;
                let payload = self.call1(self.hosts.cell_payload, scrut);
                let extra = self.call1(self.hosts.cell_extra, scrut);
                for (fname, fpat) in fields {
                    let idx = info
                        .fields
                        .iter()
                        .position(|f| f == fname)
                        .ok_or_else(|| err(format!("unknown field `{fname}`")))?;
                    let val = if idx == 0 { payload } else { extra };
                    let is_float = info.field_float.get(fname).copied().unwrap_or(false);
                    match fpat {
                        Pat::Ident { name: bn, .. }
                            if !bn.chars().next().is_some_and(|c| c.is_uppercase()) =>
                        {
                            let v = self.alloc_var();
                            self.bcx.def_var(v, val);
                            self.vars.insert(bn.clone(), v);
                            self.str_vars.insert(bn.clone(), false);
                            self.float_vars.insert(bn.clone(), is_float);
                        }
                        other => self.bind_pat(val, other)?,
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value, JitError> {
        if let Expr::Field {
            base,
            name: method,
            ..
        } = callee
        {
            let recv = self.emit_expr(base)?;
            match method.as_str() {
                "len" => return Ok(self.call1(self.hosts.str_len, recv)),
                "unwrap" => {
                    return Ok(self.call1(self.hosts.cell_payload, recv));
                }
                "sqrt" => return Ok(self.call1(self.hosts.fsqrt, recv)),
                _ => {
                    let ty = self.expr_struct_type(base).ok_or_else(|| {
                        err(format!("cannot resolve receiver type for `{method}`"))
                    })?;
                    let key = format!("{ty}.{method}");
                    let fid = self
                        .func_ids
                        .get(&key)
                        .ok_or_else(|| err(format!("unknown method `{key}`")))?;
                    let mut arg_vs = vec![recv];
                    for a in args {
                        arg_vs.push(self.emit_expr(a)?);
                    }
                    let fref = self.module.declare_func_in_func(*fid, self.bcx.func);
                    let call = self.bcx.ins().call(fref, &arg_vs);
                    return Ok(self.bcx.inst_results(call)[0]);
                }
            }
        }

        let Expr::Ident { name, .. } = callee else {
            return Err(err("call target must be a name"));
        };
        match name.as_str() {
            "print" => {
                let a = args
                    .first()
                    .ok_or_else(|| err("print needs an argument"))?;
                let v = self.emit_expr(a)?;
                let fid = if self.expr_is_str(a) {
                    self.hosts.print_s
                } else if self.expr_is_float(a) {
                    self.hosts.print_f
                } else {
                    self.hosts.print_i
                };
                let fref = self.module.declare_func_in_func(fid, self.bcx.func);
                self.bcx.ins().call(fref, &[v]);
                Ok(self.bcx.ins().iconst(types::I64, 0))
            }
            "Some" => {
                let a = args.first().ok_or_else(|| err("Some needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.make_some, v))
            }
            "Ok" => {
                let a = args.first().ok_or_else(|| err("Ok needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.make_ok, v))
            }
            "Err" => {
                let a = args.first().ok_or_else(|| err("Err needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.make_err, v))
            }
            "read_file" => {
                let a = args.first().ok_or_else(|| err("read_file needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.read_file, v))
            }
            "write_file" => {
                if args.len() != 2 {
                    return Err(err("write_file needs 2 args"));
                }
                let p = self.emit_expr(&args[0])?;
                let c = self.emit_expr(&args[1])?;
                Ok(self.call2(self.hosts.write_file, p, c))
            }
            "getenv" => {
                let a = args.first().ok_or_else(|| err("getenv needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.getenv, v))
            }
            "argc" => Ok(self.call0(self.hosts.argc)),
            "arg" => {
                let a = args.first().ok_or_else(|| err("arg needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.arg, v))
            }
            "exit" => {
                let a = args.first().ok_or_else(|| err("exit needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.exit, v))
            }
            "sleep_ms" => {
                let a = args.first().ok_or_else(|| err("sleep_ms needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.sleep_ms, v))
            }
            "now_ms" => Ok(self.call0(self.hosts.now_ms)),
            "eprint" => {
                let a = args.first().ok_or_else(|| err("eprint needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                Ok(self.call1(self.hosts.eprint, v))
            }
            "gc" => Ok(self.call0(self.hosts.gc)),
            "abs" => {
                let a = args.first().ok_or_else(|| err("abs needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                let zero = self.bcx.ins().iconst(types::I64, 0);
                let neg = self.bcx.ins().ineg(v);
                let is_neg = self.bcx.ins().icmp(IntCC::SignedLessThan, v, zero);
                Ok(self.bcx.ins().select(is_neg, neg, v))
            }
            "min" | "max" => {
                if args.len() != 2 {
                    return Err(err(format!("{name} needs 2 args")));
                }
                let a = self.emit_expr(&args[0])?;
                let b = self.emit_expr(&args[1])?;
                let lt = self.bcx.ins().icmp(IntCC::SignedLessThan, a, b);
                if name == "min" {
                    Ok(self.bcx.ins().select(lt, a, b))
                } else {
                    Ok(self.bcx.ins().select(lt, b, a))
                }
            }
            "assert" => {
                let a = args.first().ok_or_else(|| err("assert needs 1 arg"))?;
                let v = self.emit_expr(a)?;
                let zero = self.bcx.ins().iconst(types::I64, 0);
                let ok = self.bcx.ins().icmp(IntCC::NotEqual, v, zero);
                let ok_b = self.bcx.create_block();
                let fail_b = self.bcx.create_block();
                let merge = self.bcx.create_block();
                self.bcx.ins().brif(ok, ok_b, &[], fail_b, &[]);

                self.bcx.switch_to_block(fail_b);
                self.bcx.seal_block(fail_b);
                let fref = self
                    .module
                    .declare_func_in_func(self.hosts.abort, self.bcx.func);
                self.bcx.ins().call(fref, &[]);
                let z = self.bcx.ins().iconst(types::I64, 0);
                self.bcx.ins().return_(&[z]);

                self.bcx.switch_to_block(ok_b);
                self.bcx.seal_block(ok_b);
                self.bcx.ins().jump(merge, &[]);

                self.bcx.switch_to_block(merge);
                self.bcx.seal_block(merge);
                Ok(self.bcx.ins().iconst(types::I64, 0))
            }
            _ => {
                if let Some(fid) = self.extern_ids.get(name) {
                    let mut arg_vs = Vec::new();
                    for a in args {
                        arg_vs.push(self.emit_expr(a)?);
                    }
                    let fref = self.module.declare_func_in_func(*fid, self.bcx.func);
                    let call = self.bcx.ins().call(fref, &arg_vs);
                    return Ok(self.bcx.inst_results(call)[0]);
                }
                let fid = self
                    .func_ids
                    .get(name)
                    .ok_or_else(|| err(format!("unknown function `{name}`")))?;
                let mut arg_vs = Vec::new();
                for a in args {
                    arg_vs.push(self.emit_expr(a)?);
                }
                let fref = self.module.declare_func_in_func(*fid, self.bcx.func);
                let call = self.bcx.ins().call(fref, &arg_vs);
                Ok(self.bcx.inst_results(call)[0])
            }
        }
    }
}
