//! Newton-Raphson reciprocal division, O(M(n) log n).
//!
//! Scheme: maintain `x` approximating `B^p / v` at scale `p` with a tracked
//! accuracy `acc` (relative error <= B^-acc). Each Newton step doubles
//! `acc`. The division extracts a candidate quotient `q = (u * x) >> (64p)`
//! and then exacts it with a bounded correction loop computing
//! `r = u - q*v`; the correction is exact arithmetic, so the result is
//! always correct regardless of the approximation quality (which only
//! affects how many correction steps run - a few by construction).
//!
//! The reciprocal primitive is shared with isqrt and MegaFloat (spec 6).

use crate::algos::add::{cmp_slices, sub_slices};
use crate::algos::div::schoolbook;
use crate::algos::mul;
use crate::algos::Scratch;
use crate::biguint::BigUInt;
use crate::limb::{Limb, LIMB_BITS};

/// Maximum correction steps before falling back to schoolbook (should be
/// unreachable; a safety net that keeps the function total).
const MAX_CORRECTIONS: usize = 64;

/// Newton iteration cap per reciprocal call (acc doubles each step, so
/// log2(p) + slack steps suffice; the cap keeps the function total).
fn newton_step_cap(p: usize) -> usize {
    (usize::BITS - (p.max(2) as u32).leading_zeros()) as usize + 4
}

/// Approximate `B^p / v` for canonical `v` (nonzero), p >= v.len() + 2.
/// Returns `(x, acc)` with relative error <= B^-acc (acc >= 1).
pub fn reciprocal_at(v: &[Limb], p: usize, scratch: &mut Scratch) -> (BigUInt, u32) {
    let m = v.len();
    debug_assert!(m >= 1 && *v.last().unwrap() != 0);
    debug_assert!(p >= m + 2, "reciprocal scale must exceed divisor limbs");

    // Base case at scale m+1: exact floor(B^(m+1) / v) via schoolbook.
    // Relative error <= v / B^(m+1) <= 1/B, so acc = 1.
    let base_scale = m + 1;
    let mut bm: Vec<Limb> = vec![0; base_scale];
    bm.push(1); // B^(m+1)
    let (x0, _) = schoolbook::divmod(&bm, v);
    let mut x = BigUInt::from_limbs(x0);
    let mut scale = base_scale;
    let mut acc: u32 = 1;

    // Grow the scale by doubling; each phase shifts x to the new scale and
    // runs one Newton step (relative error squares).
    while scale < p {
        let new_scale = (scale * 2).min(p);
        // x_scaled = x * B^(new_scale - scale)
        let shift = (new_scale - scale) * LIMB_BITS as usize;
        let xs = x.shl(shift);
        x = newton_refine(&xs, v, new_scale, scratch);
        scale = new_scale;
        acc = acc.saturating_mul(2);
    }

    // Fixed-scale Newton until accurate enough for direct quotient use:
    // acc >= p - 5 bounds the absolute reciprocal error by ~B^5/v, so the
    // quotient candidate error stays <= ~2 for both the Newton division
    // and the Burnikel-Ziegler 2n/n step.
    let target_acc = p.saturating_sub(5).max(2) as u32;
    let cap = newton_step_cap(p);
    let mut steps = 0;
    while acc < target_acc && steps < cap {
        x = newton_refine(&x, v, p, scratch);
        acc = acc.saturating_mul(2);
        steps += 1;
    }
    (x, acc)
}

/// One Newton step at scale `p`: x <- floor(x * (2*B^p - v*x) / B^p).
fn newton_refine(x: &BigUInt, v: &[Limb], p: usize, scratch: &mut Scratch) -> BigUInt {
    let t = BigUInt::from_limbs(mul::mul_best(v, x.limbs(), scratch)); // ~ B^p
                                                                       // w = 2*B^p - t ; t < 2*B^p holds because x <= B^p/v + 1 and v <= B^m.
    let two_bp = {
        let mut l = vec![0u64; p];
        l.push(2); // 2 * B^p
        BigUInt::from_limbs(l)
    };
    if cmp_slices(t.limbs(), two_bp.limbs()) != std::cmp::Ordering::Less {
        // Overshoot beyond the proven envelope: return x unchanged; the
        // division correction step keeps results exact.
        return x.clone();
    }
    let w = two_bp.sub(&t);
    let prod = BigUInt::from_limbs(mul::mul_best(x.limbs(), w.limbs(), scratch));
    prod.shr(p * LIMB_BITS as usize)
}

/// Newton-based division of canonical magnitudes; `v` canonical, nonzero.
/// Returns (q, r) with u = q*v + r, 0 <= r < v.
pub fn divmod(u: &[Limb], v: &[Limb], scratch: &mut Scratch) -> (Vec<Limb>, Vec<Limb>) {
    debug_assert!(!v.is_empty(), "divmod_newton requires v != 0");
    if cmp_slices(u, v) == std::cmp::Ordering::Less {
        return (Vec::new(), u.to_vec());
    }
    let n = u.len();
    let m = v.len();

    // Normalize v so its top limb has the MSB set; the same shift applied
    // to u leaves the quotient unchanged and scales the remainder by 2^s.
    let shift = LIMB_BITS - crate::limb::limb_bits(v[m - 1]);
    let shift = shift % LIMB_BITS;
    let u_s = BigUInt::from_limbs(u.to_vec()).shl(shift as usize);
    let v_s = BigUInt::from_limbs(v.to_vec()).shl(shift as usize);

    let p = n - m + 2;
    if p < m + 2 {
        // Quotient shorter than the divisor (n < 2m): the reciprocal scale
        // cannot exceed the divisor length, which is outside Newton's proven
        // envelope (reciprocal_at requires p >= m + 2). Downgrade to
        // schoolbook, which is optimal at this size. Regression: modpow with
        // a large modulus hit this band (lhs - m >= 48 but lhs < 2m) and
        // tripped the reciprocal debug assertion.
        return schoolbook::divmod(u, v);
    }
    let (x, _acc) = reciprocal_at(v_s.limbs(), p, scratch);

    // Candidate quotient.
    let prod = BigUInt::from_limbs(mul::mul_best(u_s.limbs(), x.limbs(), scratch));
    let q_cand = prod.shr(p * LIMB_BITS as usize);

    // Exact correction in the shifted domain (r_shift = u_s - q*v_s).
    let (q_shift, r_shift) = correct_shifted(&u_s, v_s.limbs(), q_cand, scratch);
    // Undo the shift on the remainder.
    let r = r_shift.shr(shift as usize);
    (q_shift.into_limbs(), r.into_limbs())
}

/// Adjust candidate q so that 0 <= u_s - q*v_s < v_s, exactly.
fn correct_shifted(
    u_s: &BigUInt,
    v_s: &[Limb],
    q_cand: BigUInt,
    scratch: &mut Scratch,
) -> (BigUInt, BigUInt) {
    let mut q = q_cand;
    let mut qv = BigUInt::from_limbs(mul::mul_best(q.limbs(), v_s, scratch));
    let mut r = if cmp_slices(qv.limbs(), u_s.limbs()) == std::cmp::Ordering::Greater {
        // q too big: step down; qv shrinks by v each time.
        let mut steps = 0;
        loop {
            q = q.sub(&BigUInt::one());
            qv = BigUInt::from_limbs(sub_slices(qv.limbs(), v_s));
            if cmp_slices(qv.limbs(), u_s.limbs()) != std::cmp::Ordering::Greater {
                break;
            }
            steps += 1;
            if steps > MAX_CORRECTIONS {
                // Safety net (unreachable in practice): exact schoolbook.
                let (q, r) = schoolbook::divmod(u_s.limbs(), v_s);
                return (BigUInt::from_limbs(q), BigUInt::from_limbs(r));
            }
        }
        BigUInt::from_limbs(sub_slices(u_s.limbs(), qv.limbs()))
    } else {
        BigUInt::from_limbs(sub_slices(u_s.limbs(), qv.limbs()))
    };
    // r >= 0 now; reduce while r >= v_s.
    let mut steps = 0;
    let vs = BigUInt::from_limbs(v_s.to_vec());
    while cmp_slices(r.limbs(), vs.limbs()) != std::cmp::Ordering::Less {
        r = BigUInt::from_limbs(sub_slices(r.limbs(), v_s));
        q = q.add(&BigUInt::one());
        steps += 1;
        if steps > MAX_CORRECTIONS {
            let (q, r) = schoolbook::divmod(u_s.limbs(), v_s);
            return (BigUInt::from_limbs(q), BigUInt::from_limbs(r));
        }
    }
    let _ = scratch;
    (q, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::div::schoolbook::divmod as sb_divmod;

    fn check(u: &[Limb], v: &[Limb]) {
        let mut scratch = Scratch::new();
        let (q, r) = divmod(u, v, &mut scratch);
        let prod = mul::mul_best(&q, v, &mut scratch);
        let sum = crate::algos::add::add_slices(&prod, &r);
        assert_eq!(sum, u.to_vec(), "q*v+r != u (n={} m={})", u.len(), v.len());
        assert!(cmp_slices(&r, v) == std::cmp::Ordering::Less, "r >= v");
    }

    fn pseudo_random_vec(len: usize, seed: u64) -> Vec<Limb> {
        let mut s = seed | 1;
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            })
            .collect()
    }

    #[test]
    fn balanced_sizes() {
        for (n, m) in [
            (60usize, 4usize),
            (100, 10),
            (200, 50),
            (257, 128),
            (300, 150),
            (512, 256),
        ] {
            let u = pseudo_random_vec(n, 0xA5A5_5A5A + n as u64);
            let mut v = pseudo_random_vec(m, 0xDEAD_BEEF + m as u64);
            while *v.last().unwrap() == 0 {
                *v.last_mut().unwrap() = 1;
            }
            check(&u, &v);
        }
    }

    #[test]
    fn unbalanced_sizes() {
        for (n, m) in [(300usize, 3usize), (1000, 10), (2000, 100), (1024, 4)] {
            let u = pseudo_random_vec(n, 0x1234_5678 + n as u64);
            let mut v = pseudo_random_vec(m, 0x9E37_79B9 + m as u64);
            while *v.last().unwrap() == 0 {
                *v.last_mut().unwrap() = 1;
            }
            check(&u, &v);
        }
    }

    #[test]
    fn quotient_shorter_than_divisor() {
        // n < 2m: the reciprocal scale p = n - m + 2 does not exceed the
        // divisor length, so divmod must downgrade to schoolbook and stay
        // exact. The (100, 51) pair sits inside the dispatcher's Newton band
        // (gap >= 48, lhs < 2m) — the exact shape that panicked before the
        // guard.
        for (n, m) in [
            (2usize, 2usize),
            (3, 3),
            (4, 3),
            (5, 4),
            (8, 5),
            (40, 30),
            (100, 51),
        ] {
            let u = pseudo_random_vec(n, 0x5EED_0000 + n as u64);
            let mut v = pseudo_random_vec(m, 0xBEEF_0000 + m as u64);
            while *v.last().unwrap() == 0 {
                *v.last_mut().unwrap() = 1;
            }
            let mut scratch = Scratch::new();
            let got = divmod(&u, &v, &mut scratch);
            let want = sb_divmod(&u, &v);
            assert_eq!(got.0, want.0, "quotient mismatch at ({n},{m})");
            assert_eq!(got.1, want.1, "remainder mismatch at ({n},{m})");
        }
    }

    #[test]
    fn agrees_with_schoolbook() {
        for (n, m) in [(80usize, 8usize), (120, 40), (250, 60)] {
            let u = pseudo_random_vec(n, n as u64);
            let mut v = pseudo_random_vec(m, m as u64 + 99);
            while *v.last().unwrap() == 0 {
                *v.last_mut().unwrap() = 1;
            }
            let mut scratch = Scratch::new();
            let got = divmod(&u, &v, &mut scratch);
            let want = sb_divmod(&u, &v);
            assert_eq!(got.0, want.0, "quotient mismatch at ({n},{m})");
            assert_eq!(got.1, want.1, "remainder mismatch at ({n},{m})");
        }
    }

    #[test]
    fn exact_division_and_remainder_range() {
        // u = v * k exactly
        let v = pseudo_random_vec(20, 77);
        let k = pseudo_random_vec(30, 88);
        let mut scratch = Scratch::new();
        let vk = mul::mul_best(&v, &k, &mut scratch);
        let (q, r) = divmod(&vk, &v, &mut scratch);
        assert_eq!(q, k);
        assert!(r.is_empty());
    }

    #[test]
    fn reciprocal_accuracy_bound() {
        let v = pseudo_random_vec(24, 0xABCD);
        let mut scratch = Scratch::new();
        let (x, acc) = reciprocal_at(&v, 40, &mut scratch);
        // Verify |B^40/v - x| is small: r = B^40 - v*x should be < ~4*v.
        let mut bp: Vec<Limb> = vec![0; 40];
        bp.push(1);
        let prod = mul::mul_best(&v, x.limbs(), &mut scratch);
        let bp_big = BigUInt::from_limbs(bp);
        let diff = if cmp_slices(bp_big.limbs(), &prod) == std::cmp::Ordering::Less {
            sub_slices(&prod, bp_big.limbs())
        } else {
            sub_slices(bp_big.limbs(), &prod)
        };
        let v_big = BigUInt::from_limbs(v.clone());
        let slack = v_big.shl(3).add(&v_big);
        assert!(
            cmp_slices(&diff, slack.limbs()) != std::cmp::Ordering::Greater,
            "reciprocal error too large (acc={acc})"
        );
        assert!(acc >= 2);
    }
}
