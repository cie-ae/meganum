//! Division algorithm family.
//!
//! * `schoolbook`: Knuth Algorithm D; baseline and small-operand path.
//! * `newton`: reciprocal-based division used for large operands; shares
//!   the Newton primitive with isqrt and MegaFloat (spec 6).
//! * `burnikel_ziegler` lands at M2 (spec section 16) and is differential
//!   tested against both paths above.

pub mod burnikel_ziegler;
pub mod newton;
pub mod schoolbook;

use crate::algos::Scratch;
use crate::limb::Limb;

use crate::dispatch::Algorithm;

/// Compute (q, r) with u = q*v + r, 0 <= r < v, for canonical magnitudes.
/// `v` must be nonzero (checked by callers). Algorithm choice comes from
/// the dispatcher (pure function of sizes + tunables + overrides).
pub fn divmod_best(u: &[Limb], v: &[Limb]) -> (Vec<Limb>, Vec<Limb>) {
    debug_assert!(!v.is_empty(), "divmod_best requires v != 0");
    if crate::algos::add::cmp_slices(u, v) == std::cmp::Ordering::Less {
        return (Vec::new(), u.to_vec());
    }
    if v.len() == 1 {
        let (q, r) = schoolbook::divmod_limb(u, v[0]);
        return (q, if r == 0 { Vec::new() } else { vec![r] });
    }
    let decision = crate::dispatch::decide(crate::dispatch::OpKind::Div, u.len(), Some(v.len()));
    match decision.algorithm {
        Algorithm::BurnikelZiegler => {
            let mut scratch = Scratch::new();
            burnikel_ziegler::divmod(u, v, &mut scratch)
        }
        Algorithm::Newton => {
            let mut scratch = Scratch::new();
            newton::divmod(u, v, &mut scratch)
        }
        _ => schoolbook::divmod(u, v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_and_large_routes_agree() {
        let u: Vec<Limb> = vec![7, 5, 3, 1, 9, 2, 4, 6, 8, 0, 1];
        let v: Vec<Limb> = vec![u64::MAX, 1, 3];
        let (q1, r1) = divmod_best(&u, &v);
        let (q2, r2) = schoolbook::divmod(&u, &v);
        assert_eq!((q1, r1), (q2, r2));
    }

    #[test]
    fn small_cases() {
        // (2^128 + 100) mod 7 = 6.
        let (q, r) = divmod_best(&[100, 0, 1], &[7]);
        assert_eq!(r, vec![6]);
        assert!(!q.is_empty());
        let (q, r) = divmod_best(&[5], &[9]);
        assert_eq!(q, Vec::<Limb>::new());
        assert_eq!(r, vec![5]);
    }
}
