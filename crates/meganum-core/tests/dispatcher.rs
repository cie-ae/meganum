//! Dispatcher behavioral tests: determinism (pure function), override
//! validation, introspection recording, and backend bit-identity (FR-3.2,
//! FR-3.3, FR-3.5).

use meganum_core::algos::mul;
use meganum_core::algos::Scratch;
use meganum_core::biguint::BigUInt;
use meganum_core::dispatch::{self, Backend, OpKind};

fn pseudo_random_vec(len: usize, seed: u64) -> Vec<u64> {
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
fn decide_is_pure_and_deterministic() {
    let d1 = dispatch::decide(OpKind::Mul, 9000, Some(9000));
    let d2 = dispatch::decide(OpKind::Mul, 9000, Some(9000));
    assert_eq!(d1, d2, "same inputs must produce the same decision");
    let small = dispatch::decide(OpKind::Mul, 10, Some(10));
    assert_eq!(small.algorithm, dispatch::Algorithm::Schoolbook);
    assert_eq!(small.backend, Backend::Sequential);
    let huge = dispatch::decide(OpKind::Mul, 9000, Some(9000));
    assert_eq!(huge.algorithm, dispatch::Algorithm::Ntt);
}

#[test]
fn overrides_change_algorithm_and_validate() {
    dispatch::set_override("mul", "karatsuba").expect("valid override");
    let d = dispatch::decide(OpKind::Mul, 9000, Some(9000));
    assert_eq!(
        d.algorithm,
        dispatch::Algorithm::Karatsuba,
        "override applied"
    );
    dispatch::reset_overrides();
    let d2 = dispatch::decide(OpKind::Mul, 9000, Some(9000));
    assert_eq!(
        d2.algorithm,
        dispatch::Algorithm::Ntt,
        "reset clears overrides"
    );
    assert!(
        dispatch::set_override("mul", "warp9").is_err(),
        "invalid algorithm"
    );
    assert!(
        dispatch::set_override("quantum", "ntt").is_err(),
        "invalid op"
    );
}

#[test]
fn introspection_records_last_decision() {
    let a = BigUInt::from_limbs(pseudo_random_vec(9000, 0xD1));
    let b = BigUInt::from_limbs(pseudo_random_vec(9000, 0xD2));
    let _ = a.mul(&b);
    let info = dispatch::last_dispatch_info().expect("decision recorded");
    assert_eq!(info.op, OpKind::Mul);
    assert_eq!(info.lhs_limbs, 9000);
    assert!(
        info.algorithm == dispatch::Algorithm::Ntt || info.algorithm == dispatch::Algorithm::Toom3
    );
}

#[test]
fn mul_routes_through_dispatcher_bands() {
    // Random values across all four mul bands; results must match the
    // reference path regardless of which algorithm the dispatcher chose.
    for (n1, n2) in [(30usize, 30), (100, 90), (1100, 900), (9000, 9000)] {
        let a = BigUInt::from_limbs(pseudo_random_vec(n1, n1 as u64 + 1));
        let b = BigUInt::from_limbs(pseudo_random_vec(n2, n2 as u64 + 2));
        let got = a.mul(&b);
        let want = {
            let mut s2 = Scratch::new();
            BigUInt::from_limbs(mul::karatsuba::mul(a.limbs(), b.limbs(), &mut s2))
        };
        // karatsuba is itself differential-tested against schoolbook/_ref.
        assert_eq!(got, want, "dispatcher-routed mul {n1}x{n2}");
    }
}

#[test]
fn parallel_and_sequential_bit_identical() {
    // Force a decision at the parallel threshold and run both backends.
    let a = BigUInt::from_limbs(pseudo_random_vec(9000, 0xBEEF));
    let b = BigUInt::from_limbs(pseudo_random_vec(9000, 0xF00D));
    let seq =
        meganum_core::algos::mul::ntt::mul(a.limbs(), b.limbs(), &mut Scratch::new(), false, 512);
    let par =
        meganum_core::algos::mul::ntt::mul(a.limbs(), b.limbs(), &mut Scratch::new(), true, 512);
    assert_eq!(seq, par, "backends must be bit-identical (FR-3.3)");
}

#[test]
fn tunables_hash_stable() {
    let h1 = dispatch::tunables_hash();
    let h2 = dispatch::tunables_hash();
    assert_eq!(h1, h2);
}
