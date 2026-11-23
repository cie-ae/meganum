//! Integer square root via Newton iteration.
//!
//! M1 ships the correct full-precision iteration with exact +/- correction.
//! M2 upgrades the inner loop to precision-doubling (spec 16) sharing the
//! Newton reciprocal primitive; the correctness contract (exact floor:
//! `s^2 <= n < (s+1)^2`) is fixed and differential-tested from M1 onward.

use crate::biguint::BigUInt;

/// Exact floor square root of a non-negative magnitude.
pub fn isqrt_mag(n: &BigUInt) -> BigUInt {
    debug_assert!(n.is_canonical(), "isqrt input must be canonical");
    if n.is_zero() {
        return BigUInt::zero();
    }
    let s = n.bit_length();
    // Small values: u128 Newton is exact and fast.
    if s <= 120 {
        let mut v: u128 = 0;
        for &l in n.limbs().iter().rev() {
            v = (v << 64) | l as u128;
        }
        let mut x: u128 = if v < 4 {
            v
        } else {
            1u128 << ((128 - v.leading_zeros()) / 2)
        };
        loop {
            let y = (x + v / x) / 2;
            if y >= x {
                break;
            }
            x = y;
        }
        // Exact correction in u128.
        while x * x > v {
            x -= 1;
        }
        while (x + 1) * (x + 1) <= v {
            x += 1;
        }
        return BigUInt::from(x as u64);
    }

    // Large values: full-precision Newton with floor division.
    let one = BigUInt::one();
    let mut x = one.shl(s.div_ceil(2));
    let max_iters = (usize::BITS - s.leading_zeros()) as usize + 8;
    for _ in 0..max_iters {
        let (q, _) = n.divmod(&x).expect("x is nonzero by construction");
        let y = x.add(&q).shr(1);
        if y.cmp(&x) != std::cmp::Ordering::Less {
            break;
        }
        x = y;
    }
    // Exact floor correction.
    while x.mul(&x).cmp(n) == std::cmp::Ordering::Greater {
        x = x.sub(&one);
    }
    loop {
        let x1 = x.add(&one);
        if x1.mul(&x1).cmp(n) != std::cmp::Ordering::Greater {
            x = x1;
        } else {
            break;
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_exact() {
        assert_eq!(isqrt_mag(&BigUInt::zero()), BigUInt::zero());
        assert_eq!(isqrt_mag(&BigUInt::one()), BigUInt::one());
        assert_eq!(isqrt_mag(&BigUInt::from(3u64)), BigUInt::one());
        assert_eq!(isqrt_mag(&BigUInt::from(4u64)), BigUInt::from(2u64));
        assert_eq!(isqrt_mag(&BigUInt::from(8u64)), BigUInt::from(2u64));
        assert_eq!(isqrt_mag(&BigUInt::from(9u64)), BigUInt::from(3u64));
    }

    #[test]
    fn perfect_squares_and_neighbors() {
        for k in [10u64, 32, 63, 64, 100, 127, 200] {
            let sq = BigUInt::from(k).mul(&BigUInt::from(k));
            assert_eq!(isqrt_mag(&sq), BigUInt::from(k), "isqrt({k}^2)");
            assert_eq!(isqrt_mag(&sq.sub(&BigUInt::one())), BigUInt::from(k - 1));
            assert_eq!(isqrt_mag(&sq.add(&BigUInt::one())), BigUInt::from(k));
        }
    }

    #[test]
    fn large_value_exactness() {
        // (2^200)^2 = 2^400; check around the boundary.
        let x = BigUInt::one().shl(200);
        let sq = x.mul(&x);
        assert_eq!(isqrt_mag(&sq), x);
        assert_eq!(isqrt_mag(&sq.sub(&BigUInt::one())), x.sub(&BigUInt::one()));
    }

    #[test]
    fn floor_property_random() {
        let mut s: u64 = 0x51ED_2701;
        for _ in 0..64 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let len = (s % 40) as usize + 1;
            let limbs: Vec<u64> = (0..len)
                .map(|i| {
                    let mut t = s ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    t ^= t >> 33;
                    t
                })
                .collect();
            let n = BigUInt::from_limbs(limbs);
            let r = isqrt_mag(&n);
            let rp1 = r.add(&BigUInt::one());
            assert!(r.mul(&r).cmp(&n) != std::cmp::Ordering::Greater);
            assert!(rp1.mul(&rp1).cmp(&n) == std::cmp::Ordering::Greater);
        }
    }
}
