//! Modular exponentiation (Montgomery multiplication for odd moduli) and
//! modular inverses (spec FR-1.4).

use crate::algos::div::divmod_best;
use crate::algos::mul;
use crate::algos::Scratch;
use crate::biguint::BigUInt;
use crate::limb::Limb;

/// Montgomery context for an odd modulus (one-limb-friendly for any size;
/// REDC with u128 intermediates per limb chain is implemented in `mont_mul`
/// on full operand vectors via the classic interleaved form).
struct MontCtx {
    /// Odd modulus, canonical.
    m: BigUInt,
    /// -m^{-1} mod 2^64
    nprime: Limb,
    /// R mod m where R = 2^(64 * m.len())
    r_mod: BigUInt,
    /// R^2 mod m
    r2_mod: BigUInt,
}

impl MontCtx {
    fn new(m: BigUInt) -> MontCtx {
        debug_assert!(
            !m.is_zero() && !m.is_even(),
            "Montgomery modulus must be odd"
        );
        let len = m.len();
        let mut nprime: Limb = 1;
        for _ in 0..6 {
            nprime = nprime.wrapping_mul(2u64.wrapping_sub(m.limbs()[0].wrapping_mul(nprime)));
        }
        let nprime = nprime.wrapping_neg();
        // R mod m: R = 2^(64*len) is larger than m, compute via repeated
        // doubling mod m (len * 64 doublings).
        let mut r_mod = BigUInt::from(2u64);
        for _ in 1..(len * 64) {
            r_mod = r_mod.add(&r_mod);
            if r_mod.cmp(&m) != std::cmp::Ordering::Less {
                r_mod = r_mod.sub(&m);
            }
        }
        // R^2 mod m: square R len*64 times? Too slow - use doubling from r_mod.
        let mut r2 = r_mod.clone();
        let mut bits = len * 64;
        while bits > 0 {
            r2 = r2.add(&r2);
            if r2.cmp(&m) != std::cmp::Ordering::Less {
                r2 = r2.sub(&m);
            }
            bits -= 1;
        }
        MontCtx {
            m,
            nprime,
            r_mod,
            r2_mod: r2,
        }
    }

    /// REDC: returns t * R^{-1} mod m for t < m^2 (t as two-multiplicand
    /// product is computed by callers via full multiplication; here we take
    /// the already-multiplied value in montgomery form and reduce one step).
    fn redc(&self, t: &BigUInt) -> BigUInt {
        let len = self.m.len();
        let mut tv = t.limbs().to_vec();
        tv.resize(2 * len + 2, 0);
        // m' = (t + n_i * m) / R with n_i = t_low * nprime mod 2^64 per limb.
        let mut acc: Vec<Limb> = tv;
        for i in 0..len {
            let ni = acc[i].wrapping_mul(self.nprime);
            if ni != 0 {
                // acc += ni * m * 2^(64 i)
                let mut carry = 0u64;
                for j in 0..len {
                    let idx = i + j;
                    let t =
                        acc[idx] as u128 + self.m.limbs()[j] as u128 * ni as u128 + carry as u128;
                    acc[idx] = t as u64;
                    carry = (t >> 64) as u64;
                }
                let mut k = i + len;
                while carry != 0 {
                    let t = acc[k] as u128 + carry as u128;
                    acc[k] = t as u64;
                    carry = (t >> 64) as u64;
                    k += 1;
                }
            }
        }
        let r = BigUInt::from_limbs(acc[len..].to_vec());
        if r.cmp(&self.m) != std::cmp::Ordering::Less {
            r.sub(&self.m)
        } else {
            r
        }
    }

    /// Montgomery form of x (x < m).
    fn to_mont(&self, x: &BigUInt, scratch: &mut Scratch) -> BigUInt {
        let prod = mul::mul_best(x.limbs(), self.r2_mod.limbs(), scratch);
        self.redc(&BigUInt::from_limbs(prod))
    }

    /// Value from Montgomery form.
    #[allow(clippy::wrong_self_convention)]
    fn from_mont(&self, x: &BigUInt, scratch: &mut Scratch) -> BigUInt {
        let prod = mul::mul_best(x.limbs(), &[1], scratch);
        self.redc(&BigUInt::from_limbs(prod))
    }

    /// Montgomery multiplication: a*b*R mod m, inputs and output in
    /// Montgomery form.
    fn mul(&self, a: &BigUInt, b: &BigUInt, scratch: &mut Scratch) -> BigUInt {
        let prod = mul::mul_best(a.limbs(), b.limbs(), scratch);
        self.redc(&BigUInt::from_limbs(prod))
    }
}

/// modpow(base^exp mod modulus) for canonical magnitudes; modulus nonzero.
/// Uses Montgomery multiplication for odd moduli, plain reduce-per-step
/// otherwise. Exponent is a u64 (the API layer splits big exponents into
/// repeated squaring chains when needed).
pub fn modpow_u64(base: &BigUInt, exp: u64, modulus: &BigUInt) -> BigUInt {
    debug_assert!(!modulus.is_zero());
    if modulus.is_even() {
        return modpow_plain(base, exp, modulus);
    }
    let ctx = MontCtx::new(modulus.clone());
    let mut scratch = Scratch::new();
    let base_m = if base.cmp(modulus) == std::cmp::Ordering::Less {
        ctx.to_mont(base, &mut scratch)
    } else {
        let (_, r) = divmod_best(base.limbs(), modulus.limbs());
        ctx.to_mont(&BigUInt::from_limbs(r), &mut scratch)
    };
    let mut acc = ctx.r_mod.clone(); // 1 in Montgomery form
    let mut e = exp;
    let mut square = base_m;
    while e > 0 {
        if e & 1 == 1 {
            acc = ctx.mul(&acc, &square, &mut scratch);
        }
        e >>= 1;
        if e > 0 {
            square = ctx.mul(&square, &square, &mut scratch);
        }
    }
    ctx.from_mont(&acc, &mut scratch)
}

/// modpow with an arbitrary-precision exponent (bit-scan square and
/// multiply); Montgomery path for odd moduli.
pub fn modpow_big(
    base: &BigUInt,
    exp: &BigUInt,
    modulus: &BigUInt,
    scratch: &mut Scratch,
) -> BigUInt {
    debug_assert!(!modulus.is_zero());
    if exp.is_zero() {
        return BigUInt::one();
    }
    let montgomery = !modulus.is_even();
    let (base_r, _) = if base.cmp(modulus) == std::cmp::Ordering::Less {
        (base.clone(), ())
    } else {
        let (_, r) = divmod_best(base.limbs(), modulus.limbs());
        (BigUInt::from_limbs(r), ())
    };
    let (mut acc, base_m);
    if montgomery {
        let ctx = MontCtx::new(modulus.clone());
        acc = ctx.r_mod.clone(); // 1 in Montgomery form
        base_m = ctx.to_mont(&base_r, scratch);
    } else {
        acc = BigUInt::one();
        base_m = base_r;
    }
    // Textbook MSB-first square-and-multiply: square every bit, multiply
    // when the bit is set.
    let nbits = exp.bit_length();
    for i in (0..nbits).rev() {
        if montgomery {
            let ctx = MontCtx::new(modulus.clone());
            acc = ctx.mul(&acc, &acc, scratch);
            if exp.bit(i) {
                acc = ctx.mul(&acc, &base_m, scratch);
            }
        } else {
            acc = BigUInt::from_limbs(mul::mul_best(acc.limbs(), acc.limbs(), scratch));
            let (_, r) = divmod_best(acc.limbs(), modulus.limbs());
            acc = BigUInt::from_limbs(r);
            if exp.bit(i) {
                acc = BigUInt::from_limbs(mul::mul_best(acc.limbs(), base_m.limbs(), scratch));
                let (_, r) = divmod_best(acc.limbs(), modulus.limbs());
                acc = BigUInt::from_limbs(r);
            }
        }
    }
    if montgomery {
        let ctx = MontCtx::new(modulus.clone());
        ctx.from_mont(&acc, scratch)
    } else {
        acc
    }
}

/// Plain square-and-multiply with a division per step (even moduli).
fn modpow_plain(base: &BigUInt, exp: u64, modulus: &BigUInt) -> BigUInt {
    let mut scratch = Scratch::new();
    let (_, b0) = divmod_best(base.limbs(), modulus.limbs());
    let mut acc = BigUInt::one();
    let mut e = exp;
    let mut sq = BigUInt::from_limbs(b0);
    while e > 0 {
        if e & 1 == 1 {
            let p = mul::mul_best(acc.limbs(), sq.limbs(), &mut scratch);
            let (_, r) = divmod_best(&p, modulus.limbs());
            acc = BigUInt::from_limbs(r);
        }
        e >>= 1;
        if e > 0 {
            sq = BigUInt::from_limbs(mul::mul_best(sq.limbs(), sq.limbs(), &mut scratch));
            let (_, r) = divmod_best(sq.limbs(), modulus.limbs());
            sq = BigUInt::from_limbs(r);
        }
    }
    acc
}

/// Modular inverse via the extended Euclidean algorithm with coefficients
/// reduced into [0, modulus) at every step (no signed bookkeeping needed).
/// Returns None when gcd(base, modulus) != 1.
pub fn mod_inverse(base: &BigUInt, modulus: &BigUInt) -> Option<BigUInt> {
    if modulus.is_zero() {
        return None;
    }
    let mut scratch = Scratch::new();
    let b = {
        let (_, r) = divmod_best(base.limbs(), modulus.limbs());
        BigUInt::from_limbs(r)
    };
    if b.is_zero() {
        return None;
    }
    let mut old_r = b;
    let mut r = modulus.clone();
    let mut old_s = BigUInt::one();
    let mut s = BigUInt::zero();
    while !r.is_zero() {
        let (q, new_r) = old_r.divmod(&r).expect("r nonzero");
        // t = (old_s - q*s) mod modulus
        let prod = BigUInt::from_limbs(mul::mul_best(s.limbs(), q.limbs(), &mut scratch));
        let (_, prod) = divmod_best(prod.limbs(), modulus.limbs());
        let prod = BigUInt::from_limbs(prod);
        let next_s = if old_s.cmp(&prod) != std::cmp::Ordering::Less {
            old_s.sub(&prod)
        } else {
            modulus.sub(&prod.sub(&old_s))
        };
        old_r = std::mem::replace(&mut r, new_r);
        old_s = std::mem::replace(&mut s, next_s);
    }
    if !old_r.is_one() {
        return None;
    }
    Some(old_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random limb vector (matches the generator used
    /// by the other core test suites).
    fn pseudo_vec(n: usize, seed: u64) -> Vec<Limb> {
        let mut s = seed | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            })
            .collect()
    }

    /// Obviously-correct reference: MSB-first square-and-multiply built only
    /// from `BigUInt::mul` + `BigUInt::divmod`.
    fn modpow_ref(base: &BigUInt, exp: &BigUInt, modulus: &BigUInt) -> BigUInt {
        let mut acc = BigUInt::one();
        let nbits = exp.bit_length();
        for i in (0..nbits).rev() {
            acc = acc.mul(&acc);
            let (_, r) = acc.divmod(modulus).expect("modulus nonzero");
            acc = r;
            if exp.bit(i) {
                acc = acc.mul(base);
                let (_, r) = acc.divmod(modulus).expect("modulus nonzero");
                acc = r;
            }
        }
        acc
    }

    #[test]
    fn modpow_matches_reference_both_parities() {
        // Covers the Montgomery path (odd modulus) and the plain
        // division-per-step path (even modulus) across 1-, 2- and 3-limb
        // moduli; the (quotient, remainder) destructuring bug this guards
        // against produced zeros for every base >= modulus.
        for seed in 0..16u64 {
            let base = BigUInt::from_limbs(pseudo_vec(3, seed * 4 + 1));
            let e = BigUInt::from_limbs(pseudo_vec(2, seed * 4 + 2)).bit_or(&BigUInt::from(5u64));

            let m_odd = BigUInt::from_limbs(pseudo_vec(1 + (seed % 3) as usize, seed * 4 + 3))
                .bit_or(&BigUInt::one());
            let r_mont = modpow_big(&base, &e, &m_odd, &mut Scratch::new());
            assert_eq!(r_mont, modpow_ref(&base, &e, &m_odd), "mont seed {seed}");
            // The u64-exponent entry point is checked against the reference
            // with its own exponent.
            let e_u = e.limbs()[0] | 5;
            assert_eq!(
                modpow_u64(&base, e_u, &m_odd),
                modpow_ref(&base, &BigUInt::from(e_u), &m_odd),
                "mont u64 ref seed {seed}"
            );

            let m_even = BigUInt::from_limbs(pseudo_vec(1 + (seed % 3) as usize, seed * 4 + 4))
                .bit_and(&BigUInt::from(u64::MAX - 1));
            if m_even.is_zero() {
                continue;
            }
            let r_plain = modpow_big(&base, &e, &m_even, &mut Scratch::new());
            assert_eq!(r_plain, modpow_ref(&base, &e, &m_even), "plain seed {seed}");
            let e_u = e.limbs()[0] | 5;
            assert_eq!(
                modpow_u64(&base, e_u, &m_even),
                modpow_ref(&base, &BigUInt::from(e_u), &m_even),
                "plain u64 ref seed {seed}"
            );
        }
    }

    #[test]
    fn mod_inverse_roundtrip() {
        for seed in 0..16u64 {
            let m = BigUInt::from_limbs(pseudo_vec(2, seed * 2 + 41)).bit_or(&BigUInt::one());
            let base = BigUInt::from_limbs(pseudo_vec(3, seed * 2 + 42));
            let (_, rem) = base.divmod(&m).expect("m nonzero");
            if rem.is_zero() {
                continue;
            }
            if let Some(inv) = mod_inverse(&rem, &m) {
                let prod = inv.mul(&rem);
                let (_, one) = prod.divmod(&m).expect("m nonzero");
                assert_eq!(one, BigUInt::one(), "inverse seed {seed}");
            }
        }
        // Non-invertible base must yield None.
        assert_eq!(
            mod_inverse(&BigUInt::from(6u64), &BigUInt::from(9u64)),
            None
        );
        assert_eq!(mod_inverse(&BigUInt::zero(), &BigUInt::from(7u64)), None);
    }
}
