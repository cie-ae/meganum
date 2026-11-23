//! Differential tests: every optimized algorithm against its obviously-
//! correct reference twin, on random and adversarial inputs. This is the
//! primary correctness gate (spec 6 / 10).

use crate::algos::_ref;
use crate::algos::add::{add_slices, cmp_slices, sub_slices};
use crate::algos::div::{divmod_best, newton, schoolbook as div_schoolbook};
use crate::algos::mul::{karatsuba, schoolbook as mul_schoolbook};
use crate::algos::Scratch;
use crate::biguint::BigUInt;

/// xorshift PRNG (deterministic, cross-platform identical).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_limbs(rng: &mut Rng, len: usize) -> Vec<u64> {
    (0..len).map(|_| rng.next()).collect()
}

/// Adversarial limb patterns: extremes, runs, alternations.
fn adversarial_sets() -> Vec<Vec<u64>> {
    vec![
        vec![u64::MAX],
        vec![u64::MAX; 7],
        vec![0, 0, 1],
        vec![1],
        vec![u64::MAX, 0, u64::MAX],
        vec![0x8000_0000_0000_0000, 0x8000_0000_0000_0000],
        vec![0x5555_5555_5555_5555, 0xAAAA_AAAA_AAAA_AAAA],
        vec![],
        vec![0],
        vec![1, 2, 3, 4, 5, 6, 7, 8],
    ]
}

#[test]
fn add_matches_ref() {
    let mut rng = Rng::new(0xAD_01);
    for n1 in 0..40usize {
        for n2 in [0usize, 1, 2, 17, 40] {
            let a = random_limbs(&mut rng, n1);
            let b = random_limbs(&mut rng, n2);
            assert_eq!(add_slices(&a, &b), _ref::add_ref(&a, &b), "add {n1}+{n2}");
        }
    }
    for a in adversarial_sets() {
        for b in adversarial_sets() {
            assert_eq!(add_slices(&a, &b), _ref::add_ref(&a, &b), "add adversarial");
        }
    }
}

#[test]
fn sub_matches_ref() {
    let mut rng = Rng::new(0xAD_02);
    for n1 in 1..40usize {
        for n2 in [0usize, 1, 2, 17, 40] {
            let a = random_limbs(&mut rng, n1);
            let b = random_limbs(&mut rng, n2);
            if cmp_slices(&a, &b) == std::cmp::Ordering::Less {
                continue;
            }
            assert_eq!(sub_slices(&a, &b), _ref::sub_ref(&a, &b), "sub {n1}-{n2}");
        }
    }
    for a in adversarial_sets() {
        for b in adversarial_sets() {
            if cmp_slices(&a, &b) == std::cmp::Ordering::Less {
                continue;
            }
            assert_eq!(sub_slices(&a, &b), _ref::sub_ref(&a, &b), "sub adversarial");
        }
    }
}

#[test]
fn schoolbook_mul_matches_ref() {
    let mut rng = Rng::new(0xAD_03);
    for n1 in 0..25usize {
        for n2 in [0usize, 1, 3, 12, 25] {
            let a = random_limbs(&mut rng, n1);
            let b = random_limbs(&mut rng, n2);
            assert_eq!(
                mul_schoolbook::mul(&a, &b),
                _ref::mul_ref(&a, &b),
                "mul {n1}x{n2}"
            );
        }
    }
    for a in adversarial_sets() {
        for b in adversarial_sets() {
            assert_eq!(
                mul_schoolbook::mul(&a, &b),
                _ref::mul_ref(&a, &b),
                "mul adversarial"
            );
        }
    }
}

#[test]
fn karatsuba_matches_ref() {
    let mut rng = Rng::new(0xAD_04);
    for n1 in [30usize, 45, 63, 64, 65, 100, 128, 129, 200, 257] {
        for n2 in [1usize, 30, 64, n1, n1 * 2] {
            let a = random_limbs(&mut rng, n1);
            let b = random_limbs(&mut rng, n2);
            let mut scratch = Scratch::new();
            assert_eq!(
                karatsuba::mul(&a, &b, &mut scratch),
                _ref::mul_ref(&a, &b),
                "karatsuba {n1}x{n2}"
            );
        }
    }
    for a in adversarial_sets() {
        for b in adversarial_sets() {
            let mut scratch = Scratch::new();
            assert_eq!(
                karatsuba::mul(&a, &b, &mut scratch),
                _ref::mul_ref(&a, &b),
                "karatsuba adversarial"
            );
        }
    }
}

#[test]
fn schoolbook_divmod_matches_ref() {
    let mut rng = Rng::new(0xAD_05);
    for n1 in 0..20usize {
        for n2 in [1usize, 2, 3, 5] {
            if n2 > n1.max(1) {
                continue;
            }
            let a = random_limbs(&mut rng, n1.max(1));
            let mut b = random_limbs(&mut rng, n2);
            while b.last() == Some(&0) {
                *b.last_mut().unwrap() = rng.next() | 1;
            }
            let got = div_schoolbook::divmod(&a, &b);
            let want = _ref::divmod_ref(&a, &b);
            assert_eq!(got, want, "divmod {n1}/{n2}");
        }
    }
    for a in adversarial_sets() {
        for b in adversarial_sets() {
            if b.is_empty() || b.iter().all(|&x| x == 0) {
                continue;
            }
            if cmp_slices(&a, &b) == std::cmp::Ordering::Less {
                continue;
            }
            let got = div_schoolbook::divmod(&a, &b);
            let want = _ref::divmod_ref(&a, &b);
            assert_eq!(got, want, "divmod adversarial");
        }
    }
}

#[test]
fn newton_divmod_matches_ref_and_schoolbook() {
    let mut rng = Rng::new(0xAD_06);
    for (n1, n2) in [
        (50usize, 2usize),
        (60, 7),
        (70, 20),
        (100, 40),
        (120, 60),
        (200, 100),
        (260, 130),
        (300, 8),
    ] {
        let a = random_limbs(&mut rng, n1);
        let mut b = random_limbs(&mut rng, n2);
        while b.last() == Some(&0) {
            *b.last_mut().unwrap() = rng.next() | 1;
        }
        let mut scratch = Scratch::new();
        let got = newton::divmod(&a, &b, &mut scratch);
        let want = _ref::divmod_ref(&a, &b);
        assert_eq!(got, want, "newton {n1}/{n2}");
        assert_eq!(
            got,
            div_schoolbook::divmod(&a, &b),
            "newton vs schoolbook {n1}/{n2}"
        );
    }
}

#[test]
fn divmod_best_routes_consistently() {
    let mut rng = Rng::new(0xAD_07);
    for (n1, n2) in [
        (1usize, 1usize),
        (3, 2),
        (47, 3),
        (48, 4),
        (49, 5),
        (80, 10),
        (100, 100),
    ] {
        let a = random_limbs(&mut rng, n1);
        let mut b = random_limbs(&mut rng, n2);
        while b.last() == Some(&0) {
            *b.last_mut().unwrap() = rng.next() | 1;
        }
        let got = divmod_best(&a, &b);
        let want = _ref::divmod_ref(&a, &b);
        assert_eq!(got, want, "best {n1}/{n2}");
    }
}

#[test]
fn decimal_parse_format_matches_ref() {
    let mut rng = Rng::new(0xAD_08);
    for len in 1..400usize {
        let digits: String = (0..len)
            .map(|i| {
                if i == 0 && len > 1 {
                    char::from(b'1' + (rng.below(9) as u8))
                } else {
                    char::from(b'0' + (rng.below(10) as u8))
                }
            })
            .collect();
        let n = BigUInt::parse_mag_radix(&digits, 10).unwrap();
        assert_eq!(
            n.limbs(),
            _ref::from_decimal_ref(&digits),
            "parse {len} digits"
        );
        assert_eq!(
            n.to_string_radix(10).unwrap(),
            digits,
            "format {len} digits"
        );
    }
}

#[test]
fn mul_best_routes_consistently() {
    let mut rng = Rng::new(0xAD_09);
    for n in [30usize, 39, 40, 41, 80, 200] {
        let a = random_limbs(&mut rng, n);
        let b = random_limbs(&mut rng, n / 2 + 1);
        let mut scratch = Scratch::new();
        assert_eq!(
            crate::algos::mul::mul_best(&a, &b, &mut scratch),
            _ref::mul_ref(&a, &b),
            "mul_best {n}"
        );
    }
}

#[test]
fn parse_format_all_radices_roundtrip() {
    let mut rng = Rng::new(0xAD_0A);
    for radix in 2u32..=62 {
        let n = BigUInt::from_limbs(random_limbs(&mut rng, 37));
        let s = n.to_string_radix(radix).unwrap();
        let back = BigUInt::parse_mag_radix(&s, radix).unwrap();
        assert_eq!(back, n, "radix {radix} roundtrip");
    }
}

#[test]
fn isqrt_matches_ref_binary_search() {
    // Reference: bit-by-bit binary search (obviously correct, slow).
    fn isqrt_ref(n: &BigUInt) -> BigUInt {
        if n.is_zero() {
            return BigUInt::zero();
        }
        let mut best = BigUInt::zero();
        let mut bit = BigUInt::one().shl(n.bit_length());
        while bit.cmp(&BigUInt::zero()) != std::cmp::Ordering::Equal {
            let cand = best.add(&bit);
            if cand.mul(&cand).cmp(n) != std::cmp::Ordering::Greater {
                best = cand;
            }
            bit = bit.shr(1);
        }
        best
    }
    let mut rng = Rng::new(0xAD_0B);
    for len in [2usize, 5, 9, 17, 33] {
        for _ in 0..8 {
            let n = BigUInt::from_limbs(random_limbs(&mut rng, len));
            let got = crate::algos::sqrt::isqrt_mag(&n);
            assert_eq!(got, isqrt_ref(&n), "isqrt {len} limbs");
            let sq = got.mul(&got);
            let rp1 = got.add(&BigUInt::one());
            assert!(sq.cmp(&n) != std::cmp::Ordering::Greater);
            assert!(rp1.mul(&rp1).cmp(&n) == std::cmp::Ordering::Greater);
        }
    }
}

// ---------------------------------------------------------------------------
// GCD (algos::gcd): binary GCD and Euclid-with-estimated-quotients paths.
// Reference for small operands: textbook u128 Euclid (obviously correct).
// For large operands (where the u128 reference cannot reach) the two
// independent implementations cross-check each other and the result must
// divide both operands exactly (divisibility is verifiable without gcd).
// ---------------------------------------------------------------------------

/// u128 textbook Euclid; only valid while both operands fit in 128 bits.
fn gcd_u128_ref(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn limbs_of_u128(v: u128) -> Vec<u64> {
    let lo = v as u64;
    let hi = (v >> 64) as u64;
    let mut out = vec![lo, hi];
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

fn expect_gcd(a: &BigUInt, b: &BigUInt, want_limbs: Vec<u64>, ctx: &str) {
    let got = crate::algos::gcd::gcd(a, b);
    assert_eq!(got.into_limbs(), want_limbs, "gcd {ctx}");
}

#[test]
fn gcd_small_matches_u128_euclid_ref() {
    let mut rng = Rng::new(0xAD_0C);
    // Zeros, equal operands, and random small values through every entry
    // point: the dispatcher (`gcd`), `binary_gcd` directly, and `euclid_est`
    // directly (its early binary fallback is exercised by the same values).
    for _ in 0..400 {
        let la = 1 + rng.below(2);
        let lb = 1 + rng.below(2);
        let a = BigUInt::from_limbs(random_limbs(&mut rng, la));
        let b = BigUInt::from_limbs(random_limbs(&mut rng, lb));
        let va = a.limbs();
        let vb = b.limbs();
        let a128 = va
            .iter()
            .rev()
            .fold(0u128, |acc, &x| (acc << 64) | x as u128);
        let b128 = vb
            .iter()
            .rev()
            .fold(0u128, |acc, &x| (acc << 64) | x as u128);
        let want = limbs_of_u128(gcd_u128_ref(a128, b128));
        expect_gcd(&a, &b, want.clone(), "dispatch small");
        assert_eq!(
            crate::algos::gcd::binary_gcd(&a, &b).into_limbs(),
            want,
            "binary_gcd small"
        );
        assert_eq!(
            crate::algos::gcd::euclid_est(&a, &b).into_limbs(),
            want,
            "euclid_est small"
        );
    }
    let zero = BigUInt::zero();
    let one = BigUInt::one();
    expect_gcd(&zero, &zero, Vec::new(), "0,0");
    expect_gcd(&zero, &one, vec![1], "0,1");
    expect_gcd(&one, &zero, vec![1], "1,0");
    let two_pow_80 = BigUInt::one().shl(80);
    expect_gcd(&two_pow_80, &zero, two_pow_80.limbs().to_vec(), "2^80,0");
    expect_gcd(
        &two_pow_80,
        &two_pow_80,
        two_pow_80.limbs().to_vec(),
        "2^80,2^80",
    );
}

#[test]
fn gcd_large_paths_agree_and_divide() {
    let mut rng = Rng::new(0xAD_0D);
    // Sizes >= 36 limbs force the euclid_est path (its full-division branch,
    // estimated-quotient branch, and both correction branches); binary_gcd
    // is an independent implementation of the same function.
    for (na, nb) in [
        (40usize, 40usize),
        (40, 39),
        (41, 40),
        (50, 36),
        (37, 80),
        (80, 45),
    ] {
        for _ in 0..4 {
            let a = BigUInt::from_limbs(random_limbs(&mut rng, na));
            let b = BigUInt::from_limbs(random_limbs(&mut rng, nb));
            let g_bin = crate::algos::gcd::binary_gcd(&a, &b);
            let g_euc = crate::algos::gcd::euclid_est(&a, &b);
            assert_eq!(g_bin, g_euc, "binary vs euclid_est {na}x{nb}");
            if g_bin.is_zero() {
                assert!(a.is_zero() && b.is_zero(), "gcd(0,0) only when both zero");
                continue;
            }
            let (_, ra) = a.divmod(&g_bin).expect("divmod by gcd");
            assert!(ra.is_zero(), "gcd divides a ({na}x{nb})");
            let (_, rb) = b.divmod(&g_bin).expect("divmod by gcd");
            assert!(rb.is_zero(), "gcd divides b ({na}x{nb})");
        }
    }
    // Homogeneity: gcd(k*x, k*y) == k * gcd(x, y). Structured operands like
    // these are the deterministic way to drive the quotient-estimate
    // correction branches (shared leading limbs make q_est and the exact
    // quotient diverge).
    for _ in 0..12 {
        let x = BigUInt::from_limbs(random_limbs(&mut rng, 40));
        let y = BigUInt::from_limbs(random_limbs(&mut rng, 40));
        let k = BigUInt::from_limbs(vec![rng.next() | 1]);
        let a = k.mul(&x);
        let b = k.mul(&y);
        let g = crate::algos::gcd::gcd(&a, &b);
        assert_eq!(g, k.mul(&crate::algos::gcd::gcd(&x, &y)), "homogeneity");
        // And the quotient reduction: gcd(a/k, b/k) == gcd(x, y).
        let (ax, ra) = a.divmod(&k).expect("divmod by k");
        let (by, rb) = b.divmod(&k).expect("divmod by k");
        assert!(ra.is_zero(), "k divides a");
        assert!(rb.is_zero(), "k divides b");
        assert_eq!(
            crate::algos::gcd::gcd(&ax, &by),
            crate::algos::gcd::gcd(&x, &y),
            "reduction"
        );
    }
    // Adversarial limb patterns through the dispatcher (divisibility check
    // wherever both operands are nonzero).
    for sa in adversarial_sets() {
        for sb in adversarial_sets() {
            let a = BigUInt::from_limbs(sa.clone());
            let b = BigUInt::from_limbs(sb.clone());
            let g = crate::algos::gcd::gcd(&a, &b);
            if g.is_zero() {
                assert!(a.is_zero() && b.is_zero());
                continue;
            }
            if !a.is_zero() {
                let (_, ra) = a.divmod(&g).expect("divmod by gcd");
                assert!(ra.is_zero(), "gcd divides a (adversarial)");
            }
            if !b.is_zero() {
                let (_, rb) = b.divmod(&g).expect("divmod by gcd");
                assert!(rb.is_zero(), "gcd divides b (adversarial)");
            }
        }
    }
}
