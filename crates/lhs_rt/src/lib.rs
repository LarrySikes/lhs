//! C ABI for embedding the LHS interpreter in a native binary (`lhsc build`).

use std::ffi::CStr;
use std::io::{self, Write};
use std::os::raw::c_char;

use np_eval::run_program;
use np_hir::check;

/// Run an LHS source string. Returns 0 on success, non-zero on failure.
/// Diagnostics go to stderr; program output to stdout.
#[no_mangle]
pub unsafe extern "C" fn lhs_run_source(src: *const c_char) -> i32 {
    if src.is_null() {
        eprintln!("lhs_rt: null source");
        return 2;
    }
    lhs_mem::reset();
    let cstr = unsafe { CStr::from_ptr(src) };
    let text = match cstr.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            eprintln!("lhs_rt: source is not valid UTF-8");
            return 2;
        }
    };
    let (module, diags) = check("<embedded>", text);
    for d in &diags {
        eprintln!("{d}");
    }
    if !diags.is_empty() || module.is_none() {
        return 1;
    }
    let module = module.unwrap();
    match run_program(&module.program, io::stdout()) {
        Ok(_) => 0,
        Err(e) => {
            let _ = writeln!(io::stderr(), "lhs_rt: runtime error: {e}");
            1
        }
    }
}
