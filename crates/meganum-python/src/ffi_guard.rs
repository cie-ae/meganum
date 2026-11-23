//! FFI hygiene (S-1): every entry point wraps its body in `catch_unwind` so
//! no Rust panic ever crosses the FFI boundary; panics become
//! `MeganumError` subclasses. GIL-releasing helpers live here too.

use pyo3::prelude::*;

/// Convert a core error into a Python exception (see errors.rs).
pub(crate) fn map_err(err: meganum_core::error::MegaNumError) -> PyErr {
    crate::errors::to_pyerr(err)
}

/// Run `f` with the GIL released, catching panics. The closure returns a
/// PyResult computed without the GIL; payloads must be Send (FR-5.1 / S-8).
pub fn gil_release<T: Send, F: FnOnce() -> PyResult<T> + Send>(
    py: Python<'_>,
    f: F,
) -> PyResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| py.detach(f))) {
        Ok(v) => v,
        Err(panic) => Err(panic_message(panic)),
    }
}

/// Run `f` with the GIL held (cheap operations), catching panics.
pub fn gil_hold<T, F: FnOnce() -> PyResult<T>>(f: F) -> PyResult<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(panic) => Err(panic_message(panic)),
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> PyErr {
    let msg = if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "internal panic".to_string()
    };
    crate::errors::to_pyerr(meganum_core::error::MegaNumError::Internal(format!(
        "panic contained at the FFI boundary: {msg}"
    )))
}
