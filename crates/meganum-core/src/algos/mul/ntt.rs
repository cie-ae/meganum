//! Harvey-style number-theoretic transform multiplication (ADR-0003).
//!
//! Design: three NTT-friendly primes `p_i = 1 (mod 2^32)`, `2^62 <= p_i <
//! 2^63`, each with a principal 2^32-th root of unity. Operands are reduced
//! per prime, transformed iteratively (Cooley-Tukey), multiplied pointwise
//! in Montgomery form, inverse-transformed, and reconstructed with a
//! mixed-radix CRT using a 192-bit accumulator. Convolution coefficients
//! are bounded by `n * (2^64-1)^2 < 2^147` for `n <= 2^19` limbs (the 10^7
//! digit stretch goal), covered by the 3-prime modulus `>= 2^186`.
//!
//! Cyclic length `L = next_power_of_two(a.len() + b.len())` with zero
//! padding prevents wraparound. Everything is pure integer arithmetic:
//! results are bit-identical across backends and platforms.

use crate::algos::Scratch;
use crate::limb::Limb;

/// (p, principal 2^32-th root of unity) pairs. Each p is prime with
/// p = 1 (mod 2^32); verified in `tests::prime_constants`.
const PRIMES: [(Limb, Limb); 3] = [
    (0x4000_0021_0000_0001, 0x2313_482E_28B7_5412),
    (0x4000_005D_0000_0001, 0x01DF_2CEE_2BF4_910B),
    (0x4000_0063_0000_0001, 0x1414_59B7_1612_0D4F),
];

/// Montgomery constant: -p^{-1} mod 2^64 (const-computed per prime).
const fn mont_nprime(p: Limb) -> Limb {
    let mut inv: Limb = 1;
    let mut _i = 0;
    while _i < 6 {
        // Newton iteration on u64 wrapping arithmetic.
        inv = inv.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inv)));
        _i += 1;
    }
    inv.wrapping_neg()
}

/// Modular multiplication (p < 2^63, u128 product cannot overflow).
const fn mul_mod(a: Limb, b: Limb, p: Limb) -> Limb {
    ((a as u128 * b as u128) % p as u128) as Limb
}

/// a^e mod p, right-to-left square and multiply (const-capable).
const fn pow_mod(mut base: Limb, mut e: Limb, p: Limb) -> Limb {
    let mut acc: Limb = 1;
    base %= p;
    while e > 0 {
        if e & 1 == 1 {
            acc = mul_mod(acc, base, p);
        }
        base = mul_mod(base, base, p);
        e >>= 1;
    }
    acc
}

/// Precomputed per-prime constants (const so hot paths stay allocation-free).
struct PrimeConsts {
    p: Limb,
    omega: Limb,
    /// omega^{-1}
    omega_inv: Limb,
    /// -p^{-1} mod 2^64
    nprime: Limb,
    /// R mod p (Montgomery form of 1)
    one_m: Limb,
    /// R^2 mod p (for converting into Montgomery form)
    r2: Limb,
}

const fn make_consts(idx: usize) -> PrimeConsts {
    let (p, omega) = PRIMES[idx];
    let omega_inv = pow_mod(omega, p - 2, p); // prime field inverse
    let nprime = mont_nprime(p);
    // R mod p: 2^64 mod p.
    let one_m = ((1u128 << 64) % p as u128) as Limb;
    let r2 = mul_mod(one_m, one_m, p); // R^2 mod p
    PrimeConsts {
        p,
        omega,
        omega_inv,
        nprime,
        one_m,
        r2,
    }
}

const C0: PrimeConsts = make_consts(0);
const C1: PrimeConsts = make_consts(1);
const C2: PrimeConsts = make_consts(2);

const fn consts(idx: usize) -> &'static PrimeConsts {
    match idx {
        0 => &C0,
        1 => &C1,
        _ => &C2,
    }
}

/// Montgomery multiplication: returns a*b*R^{-1} mod p (REDC).
/// T = a*b < p^2 < 2^126; T + m*p < 2^127 fits u128; one conditional
/// subtraction keeps the result < p.
#[inline]
fn mont_mul(a: Limb, b: Limb, p: Limb, nprime: Limb) -> Limb {
    let t = a as u128 * b as u128;
    let m = (t as Limb).wrapping_mul(nprime);
    let mp = m as u128 * p as u128;
    let mut r = ((t + mp) >> 64) as Limb;
    if r >= p {
        r -= p;
    }
    r
}

/// Convert x (< p) into Montgomery form.
#[inline]
fn to_mont(x: Limb, c: &PrimeConsts) -> Limb {
    mont_mul(x, c.r2, c.p, c.nprime)
}

/// Convert out of Montgomery form: mont_mul(x, 1) = x * R^{-1}.
#[inline]
fn from_mont(x: Limb, c: &PrimeConsts) -> Limb {
    mont_mul(x, 1, c.p, c.nprime)
}

#[inline]
fn add_mod(a: Limb, b: Limb, p: Limb) -> Limb {
    let s = a + b;
    if s >= p {
        s - p
    } else {
        s
    }
}

#[inline]
fn sub_mod(a: Limb, b: Limb, p: Limb) -> Limb {
    let d = a.wrapping_sub(b);
    if a < b {
        d.wrapping_add(p)
    } else {
        d
    }
}

/// Twiddle table cache key: (prime index, log2(length), inverse flag).
type TwiddleCacheMap = std::collections::HashMap<(usize, u32, bool), std::sync::Arc<Vec<Limb>>>;

static TWIDDLE_CACHE: std::sync::OnceLock<std::sync::Mutex<TwiddleCacheMap>> =
    std::sync::OnceLock::new();

/// Twiddle table for length `n = 2^log_n`: entry j = omega^{j} (or omega^{-j}
/// for the inverse transform), in Montgomery form.
fn twiddles(idx: usize, log_n: u32, inverse: bool) -> std::sync::Arc<Vec<Limb>> {
    let cache =
        TWIDDLE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let key = (idx, log_n, inverse);
    if let Some(v) = cache.lock().expect("twiddle cache lock").get(&key) {
        return std::sync::Arc::clone(v);
    }
    let c = consts(idx);
    let n = 1usize << log_n;
    // Primitive n-th root: the 2^32 root raised to 2^32/n.
    let root = if inverse { c.omega_inv } else { c.omega };
    let shift = 32u32 - log_n;
    let root_for_len = pow_mod(root, 1u64 << shift, c.p);
    let mut tw = Vec::with_capacity(n / 2);
    // tw[0] = 1 in Montgomery form; tw[j] = root_for_len^j in Montgomery form.
    tw.push(c.one_m);
    if n > 2 {
        let root_m = to_mont(root_for_len, c);
        for j in 1..n / 2 {
            let prev = tw[j - 1];
            tw.push(mont_mul(prev, root_m, c.p, c.nprime));
        }
    }
    let arc = std::sync::Arc::new(tw);
    cache
        .lock()
        .expect("twiddle cache lock")
        .insert(key, std::sync::Arc::clone(&arc));
    arc
}

/// Bit-reversal permutation (standard iterative NTT pre-step).
fn bit_reverse(v: &mut [Limb]) {
    let n = v.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            v.swap(i, j);
        }
    }
}

/// In-place iterative Cooley-Tukey NTT over Montgomery-form coefficients.
/// With `parallel`, butterfly blocks at each level are processed by rayon
/// in chunks of at least `grain` limbs; block boundaries depend only on
/// (n, len, grain), so merges are deterministic and the output is
/// bit-identical to the sequential execution (FR-3.3).
fn transform(v: &mut [Limb], idx: usize, inverse: bool, parallel: bool, grain: usize) {
    let n = v.len();
    debug_assert!(n.is_power_of_two() && n >= 2);
    let log_n = n.trailing_zeros();
    let c = consts(idx);
    let tw = twiddles(idx, log_n, inverse);
    bit_reverse(v);
    let mut len = 2usize;
    while len <= n {
        let stride = n / len;
        let half = len / 2;
        if parallel && cfg!(feature = "parallel") && len >= grain.max(2) {
            #[cfg(feature = "parallel")]
            {
                use rayon::prelude::*;
                v.par_chunks_mut(len).for_each(|chunk| {
                    for j in 0..half {
                        let u = chunk[j];
                        let w = tw[j * stride];
                        let t = mont_mul(chunk[j + half], w, c.p, c.nprime);
                        chunk[j] = add_mod(u, t, c.p);
                        chunk[j + half] = sub_mod(u, t, c.p);
                    }
                });
                len <<= 1;
                continue;
            }
        }
        for block in (0..n).step_by(len) {
            for j in 0..half {
                let u = v[block + j];
                let w = tw[j * stride];
                let t = mont_mul(v[block + j + half], w, c.p, c.nprime);
                v[block + j] = add_mod(u, t, c.p);
                v[block + j + half] = sub_mod(u, t, c.p);
            }
        }
        len <<= 1;
    }
    if inverse {
        // Scale by n^{-1} (Montgomery multiply by the modular inverse of n).
        let n_mod = (n as u128 % c.p as u128) as Limb;
        let ninv = pow_mod(n_mod, c.p - 2, c.p);
        let ninv_m = to_mont(ninv, c);
        let scale = |x: Limb| mont_mul(x, ninv_m, c.p, c.nprime);
        if parallel && cfg!(feature = "parallel") {
            #[cfg(feature = "parallel")]
            {
                use rayon::prelude::*;
                v.par_iter_mut().for_each(|x| *x = scale(*x));
                return;
            }
        }
        for x in v.iter_mut() {
            *x = scale(*x);
        }
    }
}

/// Pointwise Montgomery products (parallel over ranges when requested).
fn pointwise(fa: &mut [Limb], fb: &[Limb], c: &PrimeConsts, parallel: bool, grain: usize) {
    let pair = |x: Limb, y: Limb| mont_mul(x, y, c.p, c.nprime);
    if parallel && cfg!(feature = "parallel") && fa.len() >= grain.max(2) {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            fa.par_iter_mut()
                .zip(fb.par_iter())
                .for_each(|(x, &y)| *x = pair(*x, y));
            return;
        }
    }
    for (x, &y) in fa.iter_mut().zip(fb.iter()) {
        *x = pair(*x, y);
    }
}

/// 192-bit accumulator for CRT reconstruction and carry normalization.
#[derive(Clone, Copy)]
struct U192 {
    lo: u128,
    hi: u64,
}

impl U192 {
    const ZERO: U192 = U192 { lo: 0, hi: 0 };

    #[inline]
    fn add_u128(&mut self, v: u128) {
        let (l, c) = self.lo.overflowing_add(v);
        self.lo = l;
        if c {
            self.hi += 1;
        }
    }

    #[inline]
    fn low64(&self) -> Limb {
        self.lo as Limb
    }

    #[inline]
    fn shr64(&self) -> U192 {
        let lo = (self.lo >> 64) | ((self.hi as u128) << 64);
        U192 { lo, hi: 0 }
    }

    #[inline]
    fn is_nonzero(&self) -> bool {
        self.lo != 0 || self.hi != 0
    }
}

/// Multiply two canonical limb slices via the 3-prime NTT pipeline.
/// `parallel` activates rayon execution of butterfly levels, pointwise
/// products and CRT when the dispatcher selected the Parallel backend.
pub fn mul(
    a: &[Limb],
    b: &[Limb],
    _scratch: &mut Scratch,
    parallel: bool,
    grain: usize,
) -> Vec<Limb> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let need = a.len() + b.len();
    let n = need.next_power_of_two();
    debug_assert!(n >= 2);

    // Per-prime transformed results.
    let mut res: [Vec<Limb>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (idx, res_i) in res.iter_mut().enumerate() {
        let c = consts(idx);
        let mut fa: Vec<Limb> = a.iter().map(|&x| to_mont((x % c.p) as Limb, c)).collect();
        let mut fb: Vec<Limb> = b.iter().map(|&x| to_mont((x % c.p) as Limb, c)).collect();
        fa.resize(n, 0);
        fb.resize(n, 0);
        transform(&mut fa, idx, false, parallel, grain);
        transform(&mut fb, idx, false, parallel, grain);
        pointwise(&mut fa, &fb, c, parallel, grain);
        transform(&mut fa, idx, true, parallel, grain);
        // Back to plain residues.
        for x in fa.iter_mut() {
            *x = from_mont(*x, c);
        }
        *res_i = fa;
    }

    // CRT reconstruction (mixed radix) + carry normalization.
    let (p1, p2, p3) = (consts(0).p, consts(1).p, consts(2).p);
    let p1p2 = p1 as u128 * p2 as u128;
    let p1_inv_mod_p2 = pow_mod(p1 % p2, p2 - 2, p2);
    // (p1*p2)^{-1} mod p3: p1p2 mod p3 then inverse.
    let p1p2_mod_p3 = (p1p2 % p3 as u128) as Limb;
    let p1p2_inv_mod_p3 = pow_mod(p1p2_mod_p3, p3 - 2, p3);

    let mut out: Vec<Limb> = Vec::with_capacity(need + 2);
    let mut acc = U192::ZERO;
    // All three residue vectors have length n; zip keeps the loop index-free.
    for ((&r1, &r2), &r3) in res[0].iter().zip(&res[1]).zip(&res[2]) {
        // Mixed radix digits.
        let d2 = mul_mod(sub_mod(r2, r1, p2), p1_inv_mod_p2, p2);
        // t = r1 + p1*d2 (mod p3): p1*d2 < 2^126 fits u128.
        let p1d2 = p1 as u128 * d2 as u128;
        let t = ((r1 as u128 + p1d2) % p3 as u128) as Limb;
        let d3 = mul_mod(sub_mod(r3, t, p3), p1p2_inv_mod_p3, p3);
        // x = r1 + p1*d2 + p1p2*d3 (< 2^189; our coefficients < 2^147).
        acc.add_u128(r1 as u128);
        acc.add_u128(p1d2);
        let hi_term = mul_u128_u64(p1p2, d3);
        acc.add_u128(hi_term.0);
        acc.hi = acc.hi.wrapping_add(hi_term.1);
        out.push(acc.low64());
        acc = acc.shr64();
        debug_assert!(acc.hi == 0, "carry accumulator overflow (bound violated)");
    }
    while acc.is_nonzero() {
        out.push(acc.low64());
        acc = acc.shr64();
    }
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// (lo128, carry64) of a 128-bit x 64-bit product (<= 192 bits).
fn mul_u128_u64(a: u128, b: Limb) -> (u128, u64) {
    let a0 = a as u64;
    let a1 = (a >> 64) as u64;
    let t0 = a0 as u128 * b as u128; // bits 0..~127
    let l0 = t0 as u64;
    let c0 = (t0 >> 64) as u64;
    let t1 = a1 as u128 * b as u128 + c0 as u128; // bits 64..~190
    let l1 = t1 as u64;
    let c1 = (t1 >> 64) as u64;
    let lo = (l0 as u128) | ((l1 as u128) << 64);
    (lo, c1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::mul::schoolbook;

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
    fn prime_constants_valid() {
        for (i, &(p, omega)) in PRIMES.iter().enumerate() {
            let _ = i;
            // Deterministic Miller-Rabin with the small bases covers these
            // magnitudes (< 3.3e24) exactly.
            let p128 = p as u128;
            let small_primes = [2u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
            let mut d = p - 1;
            let mut r = 0u32;
            while d % 2 == 0 {
                d /= 2;
                r += 1;
            }
            let mut composite = false;
            'outer: for &base in &small_primes {
                let mut x = pow_mod(base, d as Limb, p) as u128;
                if x == 1 || x == p128 - 1 {
                    continue;
                }
                for _ in 0..r - 1 {
                    x = x * x % p128;
                    if x == p128 - 1 {
                        continue 'outer;
                    }
                }
                composite = true;
                break;
            }
            assert!(!composite, "p_{i} not prime");
            assert_eq!((p - 1) % (1u64 << 32), 0, "p_{i} not 1 mod 2^32");
            assert!(
                ((1u64 << 62)..(1u64 << 63)).contains(&p),
                "p_{i} out of range"
            );
            // omega has full 2^32 order.
            assert_eq!(pow_mod(omega, (1u64 << 32) as Limb, p), 1, "omega order");
            assert_ne!(
                pow_mod(omega, (1u64 << 31) as Limb, p),
                1,
                "omega order too small"
            );
        }
    }

    #[test]
    fn montgomery_roundtrip() {
        for idx in 0..3 {
            let c = consts(idx);
            for x in [1u64, 2, 12345, c.p - 1] {
                let m = to_mont(x, c);
                assert_eq!(from_mont(m, c), x % c.p, "mont roundtrip p{idx} x{x}");
                let prod = mont_mul(m, m, c.p, c.nprime);
                assert_eq!(from_mont(prod, c), mul_mod(x, x, c.p), "mont square p{idx}");
            }
        }
    }

    #[test]
    fn ntt_matches_schoolbook() {
        let mut scratch = Scratch::new();
        for (n1, n2) in [(2usize, 2usize), (5, 3), (100, 100), (300, 300), (1000, 17)] {
            let a = pseudo_random_vec(n1, 0xABC + n1 as u64);
            let b = pseudo_random_vec(n2, 0xDEF + n2 as u64);
            let got = mul(&a, &b, &mut scratch, false, 512);
            let want = schoolbook::mul(&a, &b);
            assert_eq!(got, want, "ntt {n1}x{n2}");
        }
    }

    #[test]
    fn ntt_large_matches_karatsuba() {
        let a = pseudo_random_vec(2000, 0x1357);
        let b = pseudo_random_vec(1700, 0x2468);
        let mut scratch = Scratch::new();
        let got = mul(&a, &b, &mut scratch, false, 512);
        let want = crate::algos::mul::karatsuba::mul(&a, &b, &mut scratch);
        assert_eq!(got, want, "ntt vs karatsuba 2000x1700");
    }

    #[test]
    fn ntt_exact_division_shape() {
        // v * k must divide exactly with empty remainder via mul path users.
        let v = pseudo_random_vec(300, 99);
        let k = pseudo_random_vec(300, 77);
        let mut scratch = Scratch::new();
        let vk = mul(&v, &k, &mut scratch, false, 512);
        let (q, r) = crate::algos::div::divmod_best(&vk, &v);
        assert_eq!(q, k);
        assert!(r.is_empty());
    }

    #[test]
    fn u192_accumulator_basics() {
        let mut u = U192::ZERO;
        u.add_u128(u128::MAX);
        u.add_u128(1);
        // value = 2^128
        assert_eq!(u.hi, 1);
        assert_eq!(u.lo, 0);
        let s = u.shr64();
        assert_eq!(s.lo, 1u128 << 64);
    }
}
