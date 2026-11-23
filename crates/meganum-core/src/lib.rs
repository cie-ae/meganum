#![deny(unsafe_code)]
//! meganum-core: the pure-Rust engine behind MegaNum.
//!
//! The crate is split in two strictly separated layers:
//!
//! * **Algorithm layer** ([`algos`]) - pure functions over limb slices.
//!   No threads, no SIMD intrinsics, no allocator tricks. Every optimized
//!   algorithm has an obviously-correct reference twin under [`algos`] used
//!   for differential testing.
//! * **Execution layer** ([`backend`]) - kernel implementations plus
//!   scheduling (scalar / SIMD kernels, sequential / parallel scheduling).
//!   Backends never change values: results are bit-identical regardless of
//!   which backend ran.
//!
//! The [`dispatch`] module routes every operation: it is a pure function of
//! `(operation context, tunables)` and returns `(algorithm, backend, grain)`.
//!
//! Representation (see also `docs/algorithms.md`):
//! * limb = `u64`, base 2^64, little-endian order
//! * canonical zero = empty limb vector
//! * `BigFloat` = sign x normalized significand x 2^exp, precision carried
//!   by the ambient `Context` type (lands with the MegaFloat wave, spec M4;
//!   see `docs/api-megafloat.md`)
//!
//! # Safety
//!
//! `#![deny(unsafe_code)]` is lifted only inside `backend::simd` and
//! `limb` primitives; every `unsafe` block there carries a `// SAFETY:`
//! justification and an executing test (see S-2 in the spec).

pub mod algos;
pub mod backend;
pub mod bigfloat;
pub mod bigint;
pub mod biguint;
pub mod context;
pub mod dispatch;
pub mod error;
#[allow(unsafe_code)]
pub mod limb;

/// Version of the core engine; mirrors the Cargo package version.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
