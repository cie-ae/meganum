//! Greatest common divisor: binary GCD for small operands, Euclid with
//! estimated quotients for large ones (spec 5.4 / 6).
//!
//! The large path runs Euclid steps where the quotient is estimated from
//! the 64-bit leading heads and applied as a single-limb multiply +
//! subtract (O(n) per step), then exacted with a bounded correction so the
//! result is always exact. Step counts follow the Fibonacci worst case;
//! the Lehmer cofactor-matrix batching (fewer big-number passes) is a
//! calibrated follow-up tracked in SPEC_GAPS.md - correctness is identical.

use crate::algos::add::{cmp_slices, sub_slices};
use crate::algos::mul;
use crate::algos::Scratch;
use crate::biguint::BigUInt;
use crate::limb::{limb_bits, LIMB_BITS};

/// Threshold (limbs) below which the binary GCD wins.
const BINARY_GCD_MAX_LIMBS: usize = 36;

/// GCD of two canonical magnitudes (either may be zero).
pub fn gcd(a: &BigUInt, b: &BigUInt) -> BigUInt {
    if a.is_zero() {
        return b.clone();
    }
    if b.is_zero() {
        return a.clone();
    }
    if a.len() < BINARY_GCD_MAX_LIMBS && b.len() < BINARY_GCD_MAX_LIMBS {
        binary_gcd(a, b)
    } else {
        euclid_est(a, b)
    }
}

/// Binary GCD (Stein's algorithm) on magnitudes; obviously correct.
pub fn binary_gcd(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let mut u = a.clone();
    let mut v = b.clone();
    if u.cmp(&v) == std::cmp::Ordering::Less {
        std::mem::swap(&mut u, &mut v);
    }
    if v.is_zero() {
        return u;
    }
    let tz_u = u.trailing_zeros().unwrap_or(0);
    let tz_v = v.trailing_zeros().unwrap_or(0);
    let common = tz_u.min(tz_v);
    u = u.shr(tz_u);
    v = v.shr(tz_v);
    loop {
        while u.cmp(&v) != std::cmp::Ordering::Less {
            u = u.sub(&v);
            if u.is_zero() {
                return v.shl(common);
            }
            u = u.shr(u.trailing_zeros().unwrap_or(0));
        }
        std::mem::swap(&mut u, &mut v);
    }
}

/// Euclid with estimated quotients: each step computes q from the leading
/// limbs, subtracts q*v in O(n), and corrects exactly.
pub fn euclid_est(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let mut u = a.clone();
    let mut v = b.clone();
    if u.cmp(&v) == std::cmp::Ordering::Less {
        std::mem::swap(&mut u, &mut v);
    }
    let mut scratch = Scratch::new();
    while !v.is_zero() {
        if cmp_slices(u.limbs(), v.limbs()) == std::cmp::Ordering::Less {
            std::mem::swap(&mut u, &mut v);
        }
        if v.len() < BINARY_GCD_MAX_LIMBS {
            return binary_gcd(&u, &v);
        }
        if u.len() > v.len() {
            // One full division when sizes differ by more than a limb.
            let (_, r) = u.divmod(&v).expect("v nonzero");
            u = std::mem::replace(&mut v, r);
            continue;
        }
        let n = u.len();
        // Estimate q = floor(u_head / v_head) where heads are the top limbs
        // aligned by the v shift (standard division estimate, may be off by
        // a small amount; the exact correction fixes any error).
        let shift = limb_bits(v.limbs()[v.len() - 1]);
        let u_head = ((u.limbs()[n - 1] as u128) << shift)
            | ((u.limbs()[n - 2] as u128) >> (LIMB_BITS - shift).min(LIMB_BITS));
        let v_head = ((v.limbs()[v.len() - 1] as u128) << shift)
            | ((v.limbs()[v.len() - 2] as u128) >> (LIMB_BITS - shift).min(LIMB_BITS));
        let mut q = ((u_head / v_head) as u64).max(1);
        // u -= q * v, exactly, with correction.
        let qv = mul::mul_best(v.limbs(), &[q], &mut scratch);
        if cmp_slices(&qv, u.limbs()) == std::cmp::Ordering::Greater {
            // Estimate too high: fall back to one exact division step.
            let (_, r) = u.divmod(&v).expect("v nonzero");
            u = std::mem::replace(&mut v, r);
            continue;
        }
        let mut r = sub_slices(u.limbs(), &qv);
        while cmp_slices(&r, v.limbs()) != std::cmp::Ordering::Less {
            r = sub_slices(&r, v.limbs());
            q += 1;
        }
        u = std::mem::replace(&mut v, BigUInt::from_limbs(r));
    }
    u
}
