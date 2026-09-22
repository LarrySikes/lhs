//! Shared bump arena (DECISIONS D2).
//!
//! Used by the Cranelift JIT host. AOT stubs keep a matching C bump heap with the
//! same cell layout so both paths share one memory model.

use std::sync::Mutex;

static HEAP: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Clear the bump heap (call at the start of each `main` / JIT run).
pub fn reset() {
    let mut h = HEAP.lock().unwrap();
    h.clear();
    h.reserve(1 << 20);
}

/// Allocate `n` bytes, 8-byte aligned. Panics if the 1MiB bump is exhausted.
pub unsafe fn alloc(n: usize) -> *mut u8 {
    let mut h = HEAP.lock().unwrap();
    let align = 8;
    let len = h.len();
    let pad = (align - (len % align)) % align;
    h.resize(len + pad, 0);
    let start = h.len();
    h.resize(start + n, 0);
    unsafe { h.as_mut_ptr().add(start) }
}

/// C ABI: reset bump heap.
#[no_mangle]
pub unsafe extern "C" fn lhs_heap_reset() {
    reset();
}

/// C ABI: allocate `n` bytes from the bump heap; returns pointer as integer.
#[no_mangle]
pub unsafe extern "C" fn lhs_heap_alloc(n: u64) -> i64 {
    let p = unsafe { alloc(n as usize) };
    p as i64
}

/// Bytes currently used in the bump heap (diagnostics / agents).
#[no_mangle]
pub unsafe extern "C" fn lhs_heap_used() -> u64 {
    HEAP.lock().unwrap().len() as u64
}
