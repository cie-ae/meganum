//! Dispatch introspection and overrides (spec FR-3.4 / FR-3.5).

use crate::ffi_guard::{gil_hold, map_err};
use pyo3::prelude::*;

/// Snapshot of the previous operation's dispatcher decision, as a dict:
/// `{"op": ..., "algorithm": ..., "backend": ..., "grain": ..., "lhs_limbs":
/// ..., "rhs_limbs": ..., "thresholds": [...]}` or None before any op.
#[pyfunction]
pub fn last_dispatch_info(py: Python<'_>) -> PyResult<Py<PyAny>> {
    gil_hold(|| match meganum_core::dispatch::last_dispatch_info() {
        None => Ok(py.None()),
        Some(d) => {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("op", format!("{:?}", d.op))?;
            dict.set_item("algorithm", format!("{:?}", d.algorithm))?;
            dict.set_item("backend", format!("{:?}", d.backend))?;
            dict.set_item("grain", d.grain)?;
            dict.set_item("lhs_limbs", d.lhs_limbs)?;
            dict.set_item("rhs_limbs", d.rhs_limbs)?;
            dict.set_item(
                "thresholds",
                d.thresholds_crossed
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            )?;
            Ok(dict.into_any().unbind())
        }
    })
}

/// Set validated overrides, e.g. `{"mul": "ntt"}`. Invalid keys or
/// algorithm names raise DispatchError (FR-3.5).
#[pyfunction]
pub fn set_dispatch_overrides(
    py: Python<'_>,
    overrides: std::collections::HashMap<String, String>,
) -> PyResult<()> {
    gil_hold(|| {
        for (op, algorithm) in &overrides {
            meganum_core::dispatch::set_override(op, algorithm).map_err(map_err)?;
        }
        let _ = py;
        Ok(())
    })
}

/// Clear all overrides.
#[pyfunction]
pub fn reset_dispatch_overrides() -> PyResult<()> {
    gil_hold(|| {
        meganum_core::dispatch::reset_overrides();
        Ok(())
    })
}
