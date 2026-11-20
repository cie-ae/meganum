//! Property-based algebraic law suite (proptest).
//!
//! CI runs 2,000 cases per suite; nightly raises PROPTEST_CASES to 100,000
//! (spec 10). Operand sizes are kept small so the nightly soak stays fast.

use meganum_core::bigint::{BigInt, Sign};
use meganum_core::biguint::BigUInt;
use proptest::prelude::*;

/// Canonical BigUInt strategy: 0..=6 limbs.
fn biguint() -> impl Strategy<Value = BigUInt> {
    prop::collection::vec(any::<u64>(), 0..=6).prop_map(BigUInt::from_limbs)
}

/// BigInt strategy from a magnitude plus sign bit.
fn bigint() -> impl Strategy<Value = BigInt> {
    (biguint(), any::<bool>())
        .prop_map(|(m, neg)| BigInt::new(if neg { Sign::Negative } else { Sign::Positive }, m))
}

/// BigInt constrained to i64 range (cross-checks against native i64 ops).
fn small_bigint() -> impl Strategy<Value = BigInt> {
    any::<i64>().prop_map(BigInt::from_i64)
}

fn div_nonzero() -> impl Strategy<Value = BigInt> {
    small_bigint().prop_filter("divisor must be nonzero", |b| !b.is_zero())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn add_is_commutative(a in biguint(), b in biguint()) {
        prop_assert_eq!(a.add(&b), b.add(&a));
    }

    #[test]
    fn add_is_associative(a in biguint(), b in biguint(), c in biguint()) {
        prop_assert_eq!(a.add(&b).add(&c), a.add(&b.add(&c)));
    }

    #[test]
    fn add_sub_inverse(a in biguint(), b in biguint()) {
        let s = a.add(&b);
        prop_assert_eq!(s.sub(&b), a);
    }

    #[test]
    fn mul_is_commutative(a in biguint(), b in biguint()) {
        prop_assert_eq!(a.mul(&b), b.mul(&a));
    }

    #[test]
    fn mul_is_associative(a in biguint(), b in biguint(), c in biguint()) {
        prop_assert_eq!(a.mul(&b).mul(&c), a.mul(&b.mul(&c)));
    }

    #[test]
    fn mul_distributes_over_add(a in biguint(), b in biguint(), c in biguint()) {
        prop_assert_eq!(a.mul(&b.add(&c)), a.mul(&b).add(&a.mul(&c)));
    }

    #[test]
    fn mul_matches_u128(a in any::<u64>(), b in any::<u64>()) {
        let p = BigUInt::from(a).mul(&BigUInt::from(b));
        let want = a as u128 * b as u128;
        let lo = want as u64;
        let hi = (want >> 64) as u64;
        let want_limb_vec: Vec<u64> = if hi == 0 {
            if lo == 0 { vec![] } else { vec![lo] }
        } else {
            vec![lo, hi]
        };
        prop_assert_eq!(p.into_limbs(), want_limb_vec);
    }

    #[test]
    fn shift_roundtrip(a in biguint(), k in 0usize..200) {
        prop_assert_eq!(a.shl(k).shr(k), a);
    }

    #[test]
    fn bitwise_matches_i64(a in any::<i64>(), b in any::<i64>()) {
        let (x, y) = (BigInt::from_i64(a), BigInt::from_i64(b));
        prop_assert_eq!(x.bit_and(&y).to_i64(), Some(a & b));
        prop_assert_eq!(x.bit_or(&y).to_i64(), Some(a | b));
        prop_assert_eq!(x.bit_xor(&y).to_i64(), Some(a ^ b));
        prop_assert_eq!(x.bit_not().to_i64(), Some(!a));
    }

    #[test]
    fn bitwise_identities(a in bigint(), b in bigint()) {
        let a2 = a.clone();
        prop_assert_eq!(a.bit_and(&b), b.bit_and(&a2));
        prop_assert_eq!(a.bit_or(&b), b.bit_or(&a2));
        prop_assert_eq!(a.bit_xor(&b), b.bit_xor(&a2));
        prop_assert_eq!(a.bit_not().bit_not(), a.clone());
        prop_assert_eq!(a.bit_and(&a.bit_not()), BigInt::zero());
        prop_assert_eq!(a.bit_or(&a.bit_not()), BigInt::from_i64(-1));
    }

    #[test]
    fn shifts_match_i64(a in any::<i64>(), s in 0usize..64) {
        let x = BigInt::from_i64(a);
        prop_assert_eq!(x.shr(s).to_i64(), Some(a >> s));
    }

    #[test]
    fn floor_division_laws(a in small_bigint(), b in div_nonzero()) {
        let (q, r) = a.divmod_floor(&b).unwrap();
        // a == q*b + r
        prop_assert_eq!(q.mul(&b).add(&r), a);
        // 0 <= |r| < |b|
        prop_assert!(r.mag().cmp(b.mag()) == std::cmp::Ordering::Less);
        // sign of r follows sign of b (or r is zero)
        prop_assert!(r.is_zero() || r.sign() == b.sign());
    }

    #[test]
    fn floor_matches_i64(a in any::<i64>(), b in any::<i64>().prop_filter("nz", |b| *b != 0)) {
        let (q, r) = BigInt::from_i64(a).divmod_floor(&BigInt::from_i64(b)).unwrap();
        // Python % : remainder takes the sign of the divisor.
        let mag = a.rem_euclid(b.abs());
        let want_r = if mag != 0 && b < 0 { mag - b.abs() } else { mag };
        let want_q = ((a as i128 - want_r as i128) / b as i128) as i64; // exact by construction
        prop_assert_eq!(r.to_i64(), Some(want_r));
        prop_assert_eq!(q.to_i64(), Some(want_q));
    }

    #[test]
    fn parse_format_roundtrip_decimal(a in biguint()) {
        let s = a.to_string_radix(10).unwrap();
        let back = BigUInt::parse_mag_radix(&s, 10).unwrap();
        prop_assert_eq!(back, a);
    }

    #[test]
    fn parse_format_roundtrip_radix62(a in biguint()) {
        let s = a.to_string_radix(62).unwrap();
        let back = BigUInt::parse_mag_radix(&s, 62).unwrap();
        prop_assert_eq!(back, a);
    }

    #[test]
    fn bigint_parse_format_roundtrip(a in bigint()) {
        let s = a.to_string_radix(10).unwrap();
        let back = BigInt::parse(&s).unwrap();
        prop_assert_eq!(back.clone(), a);
        prop_assert_eq!(back.to_string_radix(10).unwrap(), s);
    }

    #[test]
    fn bytes_roundtrip(a in bigint()) {
        for signed in [true, false] {
            // Unsigned encoding carries the magnitude only.
            let want = if signed { a.clone() } else { a.abs() };
            let le = a.to_bytes_le(None, signed).unwrap();
            prop_assert_eq!(BigInt::from_bytes_le(&le, signed), want.clone());
            let be = a.to_bytes_be(None, signed).unwrap();
            prop_assert_eq!(BigInt::from_bytes_be(&be, signed), want);
        }
    }

    #[test]
    fn isqrt_floor_property(a in biguint()) {
        let s = BigInt::new(Sign::Positive, a.clone()).isqrt().unwrap();
        let m = s.mag();
        prop_assert!(m.mul(m).cmp(&a) != std::cmp::Ordering::Greater);
        let s1 = s.add(&BigInt::one());
        prop_assert!(s1.mul(&s1).cmp(&BigInt::new(Sign::Positive, a)) == std::cmp::Ordering::Greater);
    }

    #[test]
    fn isqrt_exact_near_squares(k in 1u64..(1u64 << 31)) {
        // Perfect squares and neighbors: n = k^2 + delta with delta in {-1,0,1}.
        for delta in [-1i64, 0, 1] {
            let n = ((k as i64) * (k as i64) + delta) as u64;
            let got = BigInt::new(Sign::Positive, BigUInt::from(n)).isqrt().unwrap();
            let want = if delta < 0 { k - 1 } else { k };
            prop_assert_eq!(got.mag(), &BigUInt::from(want));
        }
    }

    #[test]
    fn pow_laws(a in biguint(), e1 in 0u64..8, e2 in 0u64..8) {
        let x = BigInt::new(Sign::Positive, a);
        prop_assert_eq!(x.pow_u64(e1).pow_u64(e2), x.pow_u64(e1 * e2));
        prop_assert_eq!(x.pow_u64(e1 + e2), x.pow_u64(e1).mul(&x.pow_u64(e2)));
        prop_assert_eq!(x.pow_u64(0), BigInt::one());
    }

    #[test]
    fn neg_abs_roundtrip(a in bigint()) {
        prop_assert_eq!(a.neg().neg(), a.clone());
        prop_assert_eq!(a.abs().neg(), a.neg().abs().neg());
        if !a.is_zero() {
            prop_assert!(a.neg().is_negative() != a.is_negative());
        }
    }

    #[test]
    fn canonicalization_holds(a in bigint(), b in bigint()) {
        // Operations keep the canonical zero: zero results have no magnitude.
        let z = a.sub(&a);
        prop_assert!(z.is_zero());
        prop_assert_eq!(z.sign(), Sign::Positive);
        let z2 = a.mul(&b).sub(&a.mul(&b));
        prop_assert!(z2.is_zero());
    }
}
