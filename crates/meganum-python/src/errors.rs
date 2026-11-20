//! Exception hierarchy (spec 8.4). Pythonic bases are deliberate: existing
//! `except ValueError` / `except ZeroDivisionError` user code keeps working.
//!
//! Rust can only give each native exception ONE base, but spec 8.4 requires
//! every leaf to sit under BOTH its Pythonic base and the `MeganumError`
//! root. The Python shim therefore creates dual-base subclasses
//! (`class ParseError(NativeParseError, MeganumError)`, ...) and registers
//! them back here; Rust then raises exactly the documented classes. If the
//! shim has not registered (raw embedders), the native single-base types
//! are raised as a robust fallback.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError, PyZeroDivisionError};
use pyo3::prelude::*;

create_exception!(
    meganum,
    MeganumError,
    PyException,
    "Base class for all MegaNum errors."
);
create_exception!(
    meganum,
    NativeParseError,
    PyValueError,
    "Invalid string or bytes input."
);
create_exception!(
    meganum,
    NativeDivisionByZeroError,
    PyZeroDivisionError,
    "Division or modulo by zero."
);
create_exception!(
    meganum,
    NativePrecisionError,
    PyValueError,
    "Precision request beyond the supported envelope."
);
create_exception!(
    meganum,
    NativeDispatchError,
    PyRuntimeError,
    "Invalid dispatch override configuration."
);
// Native ConversionError (ValueError-based); the shim adds MeganumError and
// OverflowError as extra bases so `isinstance(e, OverflowError)` holds.
create_exception!(
    meganum,
    NativeConversionError,
    PyValueError,
    "Lossy or out-of-range conversion."
);

/// Leaf classes registered by the Python shim (dual-base subclasses).
#[derive(Default)]
struct RegisteredLeaves {
    parse: Option<Py<PyAny>>,
    division_by_zero: Option<Py<PyAny>>,
    precision: Option<Py<PyAny>>,
    dispatch: Option<Py<PyAny>>,
    conversion: Option<Py<PyAny>>,
}

static LEAVES: std::sync::OnceLock<RegisteredLeaves> = std::sync::OnceLock::new();

/// Register the shim-level leaf classes. Called once at package import.
pub fn register_leaf_classes(
    parse: Py<PyAny>,
    division_by_zero: Py<PyAny>,
    precision: Py<PyAny>,
    dispatch: Py<PyAny>,
    conversion: Py<PyAny>,
) {
    let _ = LEAVES.set(RegisteredLeaves {
        parse: Some(parse),
        division_by_zero: Some(division_by_zero),
        precision: Some(precision),
        dispatch: Some(dispatch),
        conversion: Some(conversion),
    });
}

/// Raise a registered leaf class when available, else the native fallback.
fn leaf(py: Python<'_>, cls: Option<&Py<PyAny>>, msg: String) -> Option<PyErr> {
    cls.map(|t| {
        PyErr::from_type(
            t.bind(py)
                .clone()
                .cast_into()
                .expect("registered exception type object"),
            (msg,),
        )
    })
}

/// Map a core error onto the Python exception hierarchy (spec 8.4).
pub fn to_pyerr(err: meganum_core::error::MegaNumError) -> PyErr {
    use meganum_core::error::MegaNumError as E;
    Python::attach(|py| {
        let empty = RegisteredLeaves::default();
        let leaves = LEAVES.get().unwrap_or(&empty);
        match err {
            E::Parse(msg) => leaf(py, leaves.parse.as_ref(), msg.clone())
                .unwrap_or_else(|| PyErr::new::<NativeParseError, _>(msg)),
            E::DivisionByZero => leaf(
                py,
                leaves.division_by_zero.as_ref(),
                "division by zero".to_string(),
            )
            .unwrap_or_else(|| PyErr::new::<NativeDivisionByZeroError, _>("division by zero")),
            E::Conversion(msg) => leaf(py, leaves.conversion.as_ref(), msg.clone())
                .unwrap_or_else(|| PyErr::new::<NativeConversionError, _>(msg)),
            E::Precision(msg) => leaf(py, leaves.precision.as_ref(), msg.clone())
                .unwrap_or_else(|| PyErr::new::<NativePrecisionError, _>(msg)),
            E::Dispatch(msg) => leaf(py, leaves.dispatch.as_ref(), msg.clone())
                .unwrap_or_else(|| PyErr::new::<NativeDispatchError, _>(msg)),
            E::OutOfMemory => PyErr::new::<MeganumError, _>("out of memory"),
            // Overflow shares the Conversion leaf (both are documented as
            // `ConversionError` with bases (ValueError, OverflowError)).
            E::Overflow(msg) => leaf(py, leaves.conversion.as_ref(), msg.clone())
                .unwrap_or_else(|| PyErr::new::<NativeConversionError, _>(msg)),
            E::Internal(msg) => PyErr::new::<MeganumError, _>(format!("internal error: {msg}")),
        }
    })
}
