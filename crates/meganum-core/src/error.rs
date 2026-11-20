//! Typed error taxonomy for the whole engine (S-3: typed errors end-to-end).

use thiserror::Error;

/// Every recoverable failure inside meganum-core.
///
/// Library code never panics on bad input; it returns this type. The
/// Python bindings map each variant onto the documented exception
/// hierarchy (spec section 8.4).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MegaNumError {
    /// A string or byte payload could not be parsed.
    #[error("invalid input: {0}")]
    Parse(String),
    /// A division or modulo had a zero divisor.
    #[error("division by zero")]
    DivisionByZero,
    /// A conversion was lossy or out of range.
    #[error("conversion error: {0}")]
    Conversion(String),
    /// A precision request is beyond the supported envelope.
    #[error("precision error: {0}")]
    Precision(String),
    /// An invalid dispatch override configuration was supplied.
    #[error("dispatch error: {0}")]
    Dispatch(String),
    /// An allocation failed; degradation is graceful, never an abort.
    #[error("out of memory")]
    OutOfMemory,
    /// A value does not fit the requested representation (e.g. float range).
    #[error("overflow: {0}")]
    Overflow(String),
    /// An internal invariant was violated. This is a bug, not an input problem.
    #[error("internal invariant violated: {0}")]
    Internal(String),
}

/// Convenience alias used across the engine.
pub type Result<T> = std::result::Result<T, MegaNumError>;
