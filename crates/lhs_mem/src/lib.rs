//! Shared heap for LHS Cranelift hosts and runtimes (DECISIONS D2).
//!
//! Cells and dynamic strings use **reference counting**. A bump region still
//! backs short-lived scratch; `lhs_gc` runs a freelist sweep (drop rc==0).

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;

const BUMP_CAP: usize = 1 << 20;

static BUMP: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static USED: AtomicU64 = AtomicU64::new(0);
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
pub struct RcHeader {
    pub rc: AtomicI64,
    pub size: u64,
}

/// Clear bump scratch and reset stats (call at start of each run).
pub fn reset() {
    let mut h = BUMP.lock().unwrap();
    h.clear();
    h.reserve(BUMP_CAP);
    USED.store(0, Ordering::Relaxed);
}

/// Bump-allocate `n` bytes (8-byte aligned). For ephemeral scratch only.
pub unsafe fn bump_alloc(n: usize) -> *mut u8 {
    let mut h = BUMP.lock().unwrap();
    let align = 8;
    let len = h.len();
    let pad = (align - (len % align)) % align;
    h.resize(len + pad, 0);
    let start = h.len();
    if start + n > BUMP_CAP {
        panic!("lhs_mem: bump heap exhausted");
    }
    h.resize(start + n, 0);
    USED.store(h.len() as u64, Ordering::Relaxed);
    unsafe { h.as_mut_ptr().add(start) }
}

/// Allocate a refcounted block of `payload` bytes (plus header). RC starts at 1.
pub unsafe fn rc_alloc(payload: usize) -> *mut u8 {
    let total = std::mem::size_of::<RcHeader>() + payload;
    let layout = std::alloc::Layout::from_size_align(total, 8).unwrap();
    let p = unsafe { std::alloc::alloc_zeroed(layout) };
    if p.is_null() {
        panic!("lhs_mem: rc_alloc OOM");
    }
    let hdr = p as *mut RcHeader;
    unsafe {
        (*hdr).rc.store(1, Ordering::Relaxed);
        (*hdr).size = payload as u64;
    }
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    unsafe { p.add(std::mem::size_of::<RcHeader>()) }
}

/// Retain (+1). No-op on null.
pub unsafe fn retain(p: *mut u8) {
    if p.is_null() {
        return;
    }
    let hdr = unsafe { (p as *mut u8).sub(std::mem::size_of::<RcHeader>()) as *mut RcHeader };
    unsafe {
        (*hdr).rc.fetch_add(1, Ordering::Relaxed);
    }
}

/// Release (−1). Frees when RC hits 0.
pub unsafe fn release(p: *mut u8) {
    if p.is_null() {
        return;
    }
    let base = unsafe { (p as *mut u8).sub(std::mem::size_of::<RcHeader>()) };
    let hdr = base as *mut RcHeader;
    let prev = unsafe { (*hdr).rc.fetch_sub(1, Ordering::AcqRel) };
    if prev == 1 {
        let size = unsafe { (*hdr).size as usize };
        let total = std::mem::size_of::<RcHeader>() + size;
        let layout = std::alloc::Layout::from_size_align(total, 8).unwrap();
        unsafe { std::alloc::dealloc(base, layout) };
        FREES.fetch_add(1, Ordering::Relaxed);
    }
}

/// Copy `s` into an RC string (NUL-terminated). Caller owns one reference.
pub unsafe fn rc_cstr(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let p = unsafe { rc_alloc(bytes.len() + 1) };
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
    }
    p as i64
}

/* ---- C ABI ---- */

#[no_mangle]
pub unsafe extern "C" fn lhs_heap_reset() {
    reset();
}

#[no_mangle]
pub unsafe extern "C" fn lhs_heap_alloc(n: u64) -> i64 {
    unsafe { bump_alloc(n as usize) as i64 }
}

#[no_mangle]
pub unsafe extern "C" fn lhs_rc_alloc(n: u64) -> i64 {
    unsafe { rc_alloc(n as usize) as i64 }
}

#[no_mangle]
pub unsafe extern "C" fn lhs_retain(p: i64) {
    unsafe { retain(p as *mut u8) }
}

#[no_mangle]
pub unsafe extern "C" fn lhs_release(p: i64) {
    unsafe { release(p as *mut u8) }
}

/// GC entry: currently a no-op beyond stats (RC frees eagerly on release).
/// Kept so agents/hosts can call a stable `lhs_gc` symbol.
#[no_mangle]
pub unsafe extern "C" fn lhs_gc() -> u64 {
    FREES.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn lhs_heap_used() -> u64 {
    USED.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn lhs_heap_stats_allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn lhs_heap_stats_frees() -> u64 {
    FREES.load(Ordering::Relaxed)
}
