//! Multiplication algorithm family.
//!
//! Dispatch within the family (initial thresholds from the spec, to be
//! recalibrated by M3 benchmarks; the top-level public API routes through
//! the dispatcher at M3, which owns the final choice):
//! * `n < schoolbook_max` (40 limbs): schoolbook
//! * otherwise: Karatsuba (Toom-3 and NTT land at M2)

pub mod karatsuba;
pub mod ntt;
pub mod schoolbook;
pub mod toom3;

use crate::algos::Scratch;
use crate::dispatch::Algorithm;
use crate::limb::Limb;

/// Select and run the best multiplication for the operand sizes.
/// The algorithm choice comes from the dispatcher (`dispatch::decide`),
/// which is a pure function of (sizes, tunables) plus runtime overrides.
pub fn mul_best(a: &[Limb], b: &[Limb], scratch: &mut Scratch) -> Vec<Limb> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let decision = crate::dispatch::decide(crate::dispatch::OpKind::Mul, a.len(), Some(b.len()));
    let parallel =
        decision.backend == crate::dispatch::Backend::Parallel && cfg!(feature = "parallel");
    match decision.algorithm {
        Algorithm::Schoolbook => schoolbook::mul(a, b),
        Algorithm::Karatsuba => karatsuba::mul(a, b, scratch),
        Algorithm::Toom3 => toom3::mul(a, b, scratch),
        _ => ntt::mul(a, b, scratch, parallel, decision.grain),
    }
}
