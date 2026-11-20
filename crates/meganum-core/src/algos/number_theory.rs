//! Number-theoretic helpers: Jacobi symbol and Miller-Rabin probable-prime
//! testing (spec FR-1.4). Primality is probabilistic and documented as
//! such; small-base prechecks are deterministic for magnitudes below
//! 3.3 * 10^24.

use crate::algos::add::{add_slices, cmp_slices};
use crate::algos::div;
use crate::algos::pow;
use crate::biguint::BigUInt;
use crate::limb::Limb;

/// Jacobi symbol (a/n) for odd n >= 3; returns -1, 0 or 1 as i8.
pub fn jacobi(a: &BigUInt, n: &BigUInt) -> i8 {
    debug_assert!(!n.is_zero() && !n.is_even(), "jacobi requires odd n");
    let mut a = a.clone();
    let mut n = n.clone();
    let mut result = 1i8;
    // a mod n
    let (_, r) = a.divmod(&n).expect("n nonzero");
    a = r;
    loop {
        if a.is_zero() {
            return if n.is_one() { result } else { 0 };
        }
        // Factor out powers of 2 from a.
        let tz = a.trailing_zeros().unwrap_or(0);
        if tz > 0 {
            a = a.shr(tz);
            let n_mod_8 = n.limbs()[0] & 7;
            if tz % 2 == 1 && (n_mod_8 == 3 || n_mod_8 == 5) {
                result = -result;
            }
        }
        // Quadratic reciprocity: (a n) = (n a) * (-1)^((a-1)/2 * (n-1)/2).
        if cmp_slices(a.limbs(), n.limbs()) != std::cmp::Ordering::Less {
            let (_, r) = a.divmod(&n).expect("n nonzero");
            a = r;
            continue;
        }
        if a.is_zero() {
            return if n.is_one() { result } else { 0 };
        }
        let a_mod_4 = a.limbs()[0] & 3;
        let n_mod_4 = n.limbs()[0] & 3;
        if a_mod_4 == 3 && n_mod_4 == 3 {
            result = -result;
        }
        if a.is_one() {
            return result;
        }
        // (a n) = (n mod a, a)
        let tmp = n.clone();
        n = a.clone();
        let (_, r) = tmp.divmod(&a).expect("a nonzero after reduction");
        a = r;
    }
}

/// Deterministic Miller-Rabin witnesses valid for all n < 3.3 * 10^24.
const SMALL_BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// Cheap compositeness prechecks.
fn small_prechecks(n: &BigUInt) -> Option<bool> {
    // Some(true) = definitely prime, Some(false) = definitely composite,
    // None = run Miller-Rabin.
    if n.is_zero() || n.is_one() {
        return Some(false);
    }
    if n.is_even() {
        return Some(n.is_two());
    }
    for &p in &SMALL_BASES[..5] {
        let (_, r) = n.divmod(&BigUInt::from(p)).expect("p nonzero");
        if r.is_zero() {
            return Some(n.cmp(&BigUInt::from(p)) == std::cmp::Ordering::Equal);
        }
    }
    // Trial division by a few more small primes.
    for &p in &[41u64, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97] {
        let (_, r) = n.divmod(&BigUInt::from(p)).expect("p nonzero");
        if r.is_zero() {
            return Some(n.is_one_value(p));
        }
    }
    None
}

fn scratch_gen() -> crate::algos::Scratch {
    crate::algos::Scratch::new()
}

/// Deterministic primality for values that fit in a u64.
fn is_prime_u64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for &p in &SMALL_BASES {
        if n % p == 0 {
            return n == p;
        }
    }
    let d = n - 1;
    let r = d.trailing_zeros();
    let d = d >> r;
    for &a in &SMALL_BASES {
        let n_big = BigUInt::from(n);
        let d_big = BigUInt::from(d);
        let mut x = pow::modpow_big(&BigUInt::from(a), &d_big, &n_big, &mut scratch_gen());
        if x.is_one() || x == BigUInt::from(n - 1) {
            continue;
        }
        let mut composite = true;
        for _ in 0..r {
            x = pow::modpow_big(&x, &BigUInt::from(2u64), &n_big, &mut scratch_gen());
            if x == BigUInt::from(n - 1) {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

/// Miller-Rabin probable-prime test with `rounds` random bases after the
/// deterministic small-base prechecks. Probabilistic for magnitudes beyond
/// the deterministic witness set (documented; not a primality proof).
pub fn is_probable_prime(n: &BigUInt, rounds: u32) -> bool {
    if let Some(verdict) = small_prechecks(n) {
        return verdict;
    }
    // Deterministic for small values.
    if n.len() == 1 {
        return is_prime_u64(n.limbs()[0]);
    }
    // Write n - 1 = d * 2^r with d odd.
    let nm1 = n.sub(&BigUInt::one());
    let r = nm1.trailing_zeros().expect("n odd");
    let d = nm1.shr(r);
    // Deterministic bases first.
    for &a in &SMALL_BASES {
        if !miller_rabin_witness_pass(n, &BigUInt::from(a), &d, r) {
            return false;
        }
    }
    // Additional pseudorandom bases (xorshift-seeded; deterministic per n).
    let mut seed = n.limbs()[0] ^ (n.limbs().len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for _ in 0..rounds {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        // Base in [2, n-2]: take a random value reduced into range.
        let cand = BigUInt::from_limbs(pseudo_below(n, seed));
        let base = if cand.cmp(&BigUInt::from(2u64)) == std::cmp::Ordering::Less {
            BigUInt::from(2u64)
        } else {
            cand
        };
        if base.cmp(&nm1) != std::cmp::Ordering::Less {
            continue;
        }
        if !miller_rabin_witness_pass(n, &base, &d, r) {
            return false;
        }
    }
    true
}

/// One Miller-Rabin round: returns false when `a` witnesses compositeness.
fn miller_rabin_witness_pass(n: &BigUInt, a: &BigUInt, d: &BigUInt, r: usize) -> bool {
    let mut scratch = crate::algos::Scratch::new();
    let mut x = pow::modpow_big(a, d, n, &mut scratch);
    let nm1 = n.sub(&BigUInt::one());
    if x.is_one() || x == nm1 {
        return true;
    }
    for _ in 0..r.saturating_sub(1) {
        let sq = x.mul(&x);
        let (_, red) = sq.divmod(n).expect("n nonzero");
        x = red;
        if x == nm1 {
            return true;
        }
        if x.is_one() {
            return false;
        }
    }
    false
}

fn pseudo_below(n: &BigUInt, seed: u64) -> Vec<Limb> {
    // Deterministic pseudorandom limb vector; reduced into [2, n-2] by the
    // caller via modular reduction.
    let mut s = seed | 1;
    let mut out: Vec<Limb> = (0..n.len())
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        })
        .collect();
    // Reduce mod (n - 3), then add 2.
    let span = n.sub(&BigUInt::from(3u64));
    let (_, r) = div::divmod_best(&out, span.limbs());
    out = add_slices(&r, &[2]);
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jacobi_known_values() {
        let cases: &[(u64, u64, i8)] = &[
            (1, 5, 1),
            (2, 5, -1),
            (3, 5, -1),
            (4, 5, 1),
            (2, 7, 1),
            (3, 7, -1),
            (10, 21, -1), // (10/21): 21 = 3*7; (10/3) = 1, (10/7) = (3/7) = -1
        ];
        for &(a, n, want) in cases {
            assert_eq!(
                jacobi(&BigUInt::from(a), &BigUInt::from(n)),
                want,
                "({a}/{n})"
            );
        }
    }

    #[test]
    fn jacobi_matches_euler_for_primes() {
        // For odd prime p: (a/p) = a^((p-1)/2) mod p (Euler's criterion).
        let p = BigUInt::from(101u64);
        for a in 1u64..101 {
            let want_pow = pow::modpow_u64(&BigUInt::from(a), (101 - 1) / 2, &p);
            let expect = if want_pow.is_one() { 1 } else { -1 };
            assert_eq!(jacobi(&BigUInt::from(a), &p), expect, "({a}/101)");
        }
    }

    #[test]
    fn probable_primes_classified() {
        let primes = [
            2u64, 3, 5, 7, 11, 13, 97, 7919, 104729, 15485863, 32452843, 4294967291,
        ];
        for &p in &primes {
            assert!(is_probable_prime(&BigUInt::from(p), 8), "{p} prime");
        }
        let composites = [
            1u64, 4, 6, 9, 15, 100, 561, 1105, 1729, 41041, 825265, 4294967295,
        ];
        for &c in &composites {
            assert!(!is_probable_prime(&BigUInt::from(c), 8), "{c} composite");
        }
        // Carmichael number 561 = 3 * 11 * 17 must be composite.
        assert!(!is_probable_prime(&BigUInt::from(561u64), 8));
    }

    #[test]
    fn big_prime_and_semiprime() {
        // 2^89 - 1 is prime (Mersenne).
        let p = BigUInt::one().shl(89).sub(&BigUInt::one());
        assert!(is_probable_prime(&p, 16), "2^89 - 1 prime");
        // (2^89 - 1) * 3 is composite.
        let c = p.mul(&BigUInt::from(3u64));
        assert!(!is_probable_prime(&c, 16), "3 * (2^89-1) composite");
    }
}
