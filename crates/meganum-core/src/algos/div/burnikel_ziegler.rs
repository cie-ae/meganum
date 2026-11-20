//! Burnikel-Ziegler style divide-and-conquer division (2n/n scheme).
//!
//! Structure follows the classical BZ decomposition: the normalized divisor
//! `v` (m limbs) gets a 2m-limb reciprocal computed once with the Newton
//! primitive, the dividend is consumed in m-limb blocks (`dz` loop), and
//! each 2n/n step splits recursively. The quotient candidate from the
//! reciprocal is exacted by a bounded correction per step, so results are
//! always correct regardless of reciprocal accuracy (which only affects
//! correction counts). Differential-tested against the schoolbook and
//! Newton paths on every size band (spec 16, M2).

use crate::algos::add::{cmp_slices, sub_slices};
use crate::algos::div::{newton, schoolbook};
use crate::algos::mul;
use crate::algos::Scratch;
use crate::biguint::BigUInt;
use crate::limb::{limb_bits, Limb, LIMB_BITS};

/// Below this many limbs, the schoolbook path wins outright.
const BZ_MIN_LIMBS: usize = 64;

/// Divide canonical magnitudes with the BZ 2n/n scheme; `v` canonical
/// nonzero. Returns (q, r) with 0 <= r < v.
pub fn divmod(u: &[Limb], v: &[Limb], scratch: &mut Scratch) -> (Vec<Limb>, Vec<Limb>) {
    debug_assert!(!v.is_empty(), "bz divmod requires v != 0");
    if cmp_slices(u, v) == std::cmp::Ordering::Less {
        return (Vec::new(), u.to_vec());
    }
    let m = v.len();
    // Normalize so the top limb of v has its MSB set.
    let shift = (LIMB_BITS - limb_bits(v[m - 1])) % LIMB_BITS;
    let u_s = BigUInt::from_limbs(u.to_vec()).shl(shift as usize);
    let v_s = BigUInt::from_limbs(v.to_vec()).shl(shift as usize);
    let n = u_s.limbs().len();
    if n < m || n < BZ_MIN_LIMBS || m < 2 {
        let (q, r) = schoolbook::divmod(u, v);
        return (q, r);
    }

    // Number of m-limb blocks in the dividend (2 per recursive step).
    let mut q = vec![0u64; n - m + 1];
    let mut rem = u_s.clone();
    // dz-style loop: peel the top two blocks, divide 2m/m, shift remainder.
    let mut steps = 0;
    while rem.limbs().len() > 2 * m {
        steps += 1;
        debug_assert!(steps < 4096, "bz loop runaway");
        let len = rem.limbs().len();
        // High 2m limbs of the current remainder.
        let hi2 = rem.limbs()[len - 2 * m..].to_vec();
        let (qh, rh) = divrem2by1(&hi2, v_s.limbs(), scratch);
        // qh has at most m+1 limbs; the block value contributes at offset
        // len-2m (a possible extra top digit lands at len-m, inside q).
        let pos = len - 2 * m;
        debug_assert!(pos + qh.len() <= q.len(), "bz quotient block overflow");
        for (i, &x) in qh.iter().enumerate() {
            q[pos + i] = x;
        }
        // rem' = rh * B^{len-2m} + low: untouched low limbs at the bottom,
        // the m-limb remainder (zero-padded) at limbs len-2m..len-m.
        let mut low = rem.limbs()[..len - 2 * m].to_vec();
        let mut rh_pad = rh.clone();
        rh_pad.resize(m, 0);
        low.extend_from_slice(&rh_pad);
        rem = BigUInt::from_limbs(low);
    }
    // Final 2m/m (or smaller) step via the reciprocal path or schoolbook.
    // rem.len() is in (m, 2m]; pad with leading zeros to exactly 2m limbs
    // (value-preserving) for the reciprocal kernel.
    let (qh, rh);
    if rem.limbs().len() > m {
        // Zero-extend at the HIGH end (append zeros after the limbs);
        // prepending would multiply the value by B^k.
        let mut u2 = rem.limbs().to_vec();
        u2.resize(2 * m, 0);
        let (a, b) = divrem2by1(&u2, v_s.limbs(), scratch);
        qh = a;
        rh = b;
    } else {
        let (a, b) = schoolbook::divmod(rem.limbs(), v_s.limbs());
        qh = a;
        rh = b;
    }
    for (i, &x) in qh.iter().enumerate() {
        q[i] = x;
    }
    // Undo the shift on the remainder; quotient is shift-invariant.
    let r = BigUInt::from_limbs(rh).shr(shift as usize);
    while q.last() == Some(&0) {
        q.pop();
    }
    (q, r.into_limbs())
}

/// Divide a 2m-limb dividend by an m-limb normalized divisor using the
/// Newton reciprocal; exact via bounded correction. Returns (q: <= m+1
/// limbs, r: m limbs).
fn divrem2by1(u2: &[Limb], v: &[Limb], scratch: &mut Scratch) -> (Vec<Limb>, Vec<Limb>) {
    let m = v.len();
    debug_assert_eq!(u2.len(), 2 * m);
    // Newton reciprocal at scale 2m: x ≈ B^{2m+2}/v with small error.
    let prec = 2 * m + 2;
    let (x, _acc) = newton::reciprocal_at(v, prec, scratch);
    // Candidate quotient: floor(u2 * x / B^{2m+2}).
    let prod = BigUInt::from_limbs(mul::mul_best(u2, x.limbs(), scratch));
    let q_big = prod.shr(prec * LIMB_BITS as usize);
    // Exact correction: r = u2 - q*v; adjust q while r out of range.
    let qv = BigUInt::from_limbs(mul::mul_best(q_big.limbs(), v, scratch));
    let u_big = BigUInt::from_limbs(u2.to_vec());
    let (q_final, r) = if cmp_slices(qv.limbs(), u_big.limbs()) == std::cmp::Ordering::Greater {
        let mut q = q_big;
        let mut qv = qv;
        let mut steps = 0;
        while cmp_slices(qv.limbs(), u_big.limbs()) == std::cmp::Ordering::Greater {
            q = q.sub(&BigUInt::one());
            qv = BigUInt::from_limbs(sub_slices(qv.limbs(), v));
            steps += 1;
            debug_assert!(steps <= 8, "bz quotient correction runaway");
        }
        let r = sub_slices(u_big.limbs(), qv.limbs());
        (q, r)
    } else {
        let mut r = sub_slices(u_big.limbs(), qv.limbs());
        let mut q = q_big;
        let mut steps = 0;
        while cmp_slices(&r, v) != std::cmp::Ordering::Less {
            r = sub_slices(&r, v);
            q = q.add(&BigUInt::one());
            steps += 1;
            debug_assert!(steps <= 8, "bz quotient correction runaway");
        }
        (q, r)
    };
    debug_assert!(r.len() <= m, "bz remainder must fit m limbs");
    (q_final.into_limbs(), r)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn check(u: &[Limb], v: &[Limb], ctx: &str) {
        let mut scratch = Scratch::new();
        let got = divmod(u, v, &mut scratch);
        let prod = mul::mul_best(&got.0, v, &mut scratch);
        let sum = crate::algos::add::add_slices(&prod, &got.1);
        assert_eq!(sum, u.to_vec(), "q*v+r != u ({ctx})");
        assert!(
            crate::algos::add::cmp_slices(&got.1, v) == std::cmp::Ordering::Less,
            "r >= v ({ctx})"
        );
    }

    #[test]
    fn bz_matches_reference_paths() {
        let mut rng_state = 0xB2_0001u64;
        let mut next = || {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            rng_state
        };
        for (n, m) in [
            (130usize, 60usize),
            (200, 100),
            (256, 128),
            (300, 150),
            (400, 200),
            (512, 256),
            (600, 100),
            (700, 128),
            (900, 300),
            (1024, 512),
        ] {
            let u = pseudo_random_vec(n, next());
            let mut v = pseudo_random_vec(m, next());
            while *v.last().unwrap() == 0 {
                *v.last_mut().unwrap() |= 1;
            }
            check(&u, &v, &format!("random {n}/{m}"));
            let scratch = Scratch::new();
            let want = schoolbook::divmod(&u, &v);
            let mut scratch2 = Scratch::new();
            let got = divmod(&u, &v, &mut scratch2);
            assert_eq!(got, want, "vs schoolbook {n}/{m}");
            let _ = scratch;
        }
        // Exact division and remainder v-1 boundaries.
        for m in [64usize, 100, 200] {
            let v = pseudo_random_vec(m, next());
            let k = pseudo_random_vec(m, next());
            let mut scratch = Scratch::new();
            let vk = mul::mul_best(&v, &k, &mut scratch);
            check(&vk, &v, "exact division");
            let (q, r) = divmod(&vk, &v, &mut scratch);
            assert_eq!(q, k, "exact quotient");
            assert!(r.is_empty(), "exact remainder");
            let one_less = BigUInt::from_limbs(vk.clone())
                .sub(&BigUInt::one())
                .into_limbs();
            let (q2, r2) = divmod(&one_less, &v, &mut scratch);
            assert_eq!(
                r2.len(),
                m - v.iter().rev().take_while(|&&x| x == 0).count().min(m)
            );
            let expect_k = BigUInt::from_limbs(k).sub(&BigUInt::one());
            assert_eq!(q2, expect_k.into_limbs(), "quotient after -1");
            let _ = scratch;
        }
    }

    #[test]
    fn bz_small_and_unbalanced_fallbacks() {
        // Sizes below the BZ thresholds route to schoolbook internally and
        // must still be correct.
        let scratch = Scratch::new();
        let u = pseudo_random_vec(80, 42);
        let v = pseudo_random_vec(10, 43);
        check(&u, &v, "below threshold");
        let u2 = pseudo_random_vec(1000, 44);
        let v2 = pseudo_random_vec(4, 45);
        check(&u2, &v2, "very unbalanced");
        let _ = scratch;
    }
}

#[cfg(test)]
mod divrem_tests {
    use super::*;

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
    fn divrem2by1_constructed_inverse() {
        let mut scratch = Scratch::new();
        for m in [10usize, 20, 40] {
            for case in 0..8u64 {
                let mut v = pseudo_random_vec(m, 0x1000 + case * 7 + m as u64);
                while *v.last().unwrap() == 0 {
                    *v.last_mut().unwrap() |= 1;
                }
                let shift = (LIMB_BITS - limb_bits(v[m - 1])) % LIMB_BITS;
                let vn = BigUInt::from_limbs(v.clone())
                    .shl(shift as usize)
                    .into_limbs();
                debug_assert_eq!(vn.len(), m);
                // q_true < B^m, r < v, u2 = q_true*vn + r (exactly 2m limbs).
                let q_true = pseudo_random_vec(m, 0x2000 + case * 11 + m as u64);
                let r = pseudo_random_vec(m - 1, 0x3000 + case * 13);
                let (r, _) = schoolbook::divmod(&r, &vn);
                let mut scratch2 = Scratch::new();
                let qv = mul::mul_best(&q_true, &vn, &mut scratch2);
                let mut r_pad = r.clone();
                r_pad.resize(m, 0);
                let u2 = crate::algos::add::add_slices(&qv, &r_pad);
                let mut u2p = u2;
                u2p.resize(2 * m, 0);
                let (q, r_out) = divrem2by1(&u2p, &vn, &mut scratch);
                assert_eq!(q, q_true, "q mismatch m={m} case={case}");
                let mut r_want = r.clone();
                r_want.resize(m, 0);
                let mut r_trim = r_out.clone();
                while r_trim.last() == Some(&0) {
                    r_trim.pop();
                }
                assert_eq!(
                    r_trim,
                    {
                        let mut w = r_want;
                        while w.last() == Some(&0) {
                            w.pop();
                        }
                        w
                    },
                    "r mismatch m={m} case={case}"
                );
            }
        }
    }
}
