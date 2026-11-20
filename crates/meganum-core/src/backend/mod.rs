//! Execution backends: sequential (always), parallel (rayon), SIMD.
//!
//! Backends never change values: every kernel produces bit-identical
//! results, verified by differential tests (spec FR-3.3).

#[cfg(feature = "parallel")]
pub mod parallel;
pub mod sequential;
#[cfg(feature = "simd")]
#[allow(unsafe_code)]
pub mod simd;

/// Number of worker threads available to parallel backends:
/// `available_parallelism` (rayon's pool size once the parallel backend
/// activates). The M3 dispatcher uses this plus the worker reservation.
#[cfg(feature = "parallel")]
pub fn worker_count() -> usize {
    parallel::worker_count()
}
