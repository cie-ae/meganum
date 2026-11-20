//! PyO3 binding module `meganum._native`.
//!
//! This crate is a thin adapter: no business logic lives here. Every Rust
//! entry point passes through `ffi_guard`, which converts panics into
//! `MeganumError` subclasses so that nothing ever unwinds across the FFI
//! boundary (S-1). Compute-heavy operations release the GIL via
//! `py.detach` (PyO3 0.30 API; FR-5.1).
//!
//! Module surface (spec section 8):
//! * `MegaInt` — arbitrary-precision signed integer (see `bigint.rs`);
//! * exceptions — `MeganumError`, `ParseError`, `DivisionByZeroError`,
//!   `PrecisionError`, `DispatchError`, native `NativeConversionError`
//!   (see `errors.rs`);
//! * dispatch introspection + overrides — `last_dispatch_info`,
//!   `set_dispatch_overrides`, `reset_dispatch_overrides` (FR-3.4/3.5);
//! * pickle + shim hooks — `_unpickle_megaint`, `_register_conversion_error`;
//! * build facts — `core_version`, `about`.
//!
//! `MegaFloat`, `Context` and `Rounding` are added with the M4 wave
//! (`bigfloat.rs`/`context.rs` in the core are still placeholders).

use pyo3::prelude::*;

mod about;
mod bigint;
mod dispatch_api;
mod errors;
mod ffi_guard;

#[pyfunction]
fn core_version() -> &'static str {
    meganum_core::CORE_VERSION
}

/// Pickle support: module-level loader used by `MegaInt.__reduce__`.
#[pyfunction]
fn _unpickle_megaint(payload: &[u8]) -> PyResult<bigint::MegaInt> {
    bigint::unpickle_megaint(payload)
}

/// Shim hook: register the dual-base leaf exception classes created in the
/// Python shim (`class ParseError(NativeParseError, MeganumError)`, etc.).
/// Once registered, Rust raises exactly the documented spec 8.4 classes;
/// without registration the native single-base types are the fallback.
#[pyfunction]
fn _register_error_classes(
    parse: Py<PyAny>,
    division_by_zero: Py<PyAny>,
    precision: Py<PyAny>,
    dispatch: Py<PyAny>,
    conversion: Py<PyAny>,
) -> PyResult<()> {
    errors::register_leaf_classes(parse, division_by_zero, precision, dispatch, conversion);
    Ok(())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Spec 5.7 asks for prepare_freethreaded_python() at import time. pyo3
    // 0.30 removed that call: for extension modules the interpreter is
    // guaranteed to be initialized before the module function runs, which
    // satisfies the same contract (see ADR-0001).
    m.add_function(wrap_pyfunction!(core_version, m)?)?;
    m.add_function(wrap_pyfunction!(about::about, m)?)?;

    // Exceptions (spec 8.4). The shim subclasses the native conversion
    // error and registers the concrete class back into this module.
    m.add("MeganumError", m.py().get_type::<errors::MeganumError>())?;
    m.add(
        "NativeParseError",
        m.py().get_type::<errors::NativeParseError>(),
    )?;
    m.add(
        "NativeDivisionByZeroError",
        m.py().get_type::<errors::NativeDivisionByZeroError>(),
    )?;
    m.add(
        "NativePrecisionError",
        m.py().get_type::<errors::NativePrecisionError>(),
    )?;
    m.add(
        "NativeDispatchError",
        m.py().get_type::<errors::NativeDispatchError>(),
    )?;
    m.add(
        "NativeConversionError",
        m.py().get_type::<errors::NativeConversionError>(),
    )?;

    // Types + module-level functions.
    m.add_class::<bigint::MegaInt>()?;
    m.add_function(wrap_pyfunction!(_unpickle_megaint, m)?)?;
    m.add_function(wrap_pyfunction!(_register_error_classes, m)?)?;
    m.add_function(wrap_pyfunction!(dispatch_api::last_dispatch_info, m)?)?;
    m.add_function(wrap_pyfunction!(dispatch_api::set_dispatch_overrides, m)?)?;
    m.add_function(wrap_pyfunction!(dispatch_api::reset_dispatch_overrides, m)?)?;
    Ok(())
}
