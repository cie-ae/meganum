//! Obviously-correct reference implementations, compiled only for tests
//! (`cfg(any(test, feature = "ref-impls"))`). CI differential-tests every
//! optimized algorithm against these on random and adversarial inputs -
//! the primary correctness gate (spec 6).

use crate::limb::Limb;

/// Naive multiplication: 128-bit products rippled in with a slot-adder.
pub fn mul_ref(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    let mut acc = vec![0u64; a.len() + b.len() + 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            let (lo, hi) = crate::limb::mul_wide(x, y);
            add_slot(&mut acc, i + j, lo);
            add_slot(&mut acc, i + j + 1, hi);
        }
    }
    while acc.last() == Some(&0) {
        acc.pop();
    }
    acc
}

/// Ripple `v` into acc starting at limb `k`.
fn add_slot(acc: &mut [u64], mut k: usize, mut v: u64) {
    while v != 0 {
        debug_assert!(k < acc.len(), "mul_ref accumulator overflow");
        let t = acc[k] as u128 + v as u128;
        acc[k] = t as u64;
        v = (t >> 64) as u64;
        k += 1;
    }
}

/// Naive addition.
pub fn add_ref(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0u64;
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0) as u128;
        let y = b.get(i).copied().unwrap_or(0) as u128;
        let t = x + y + carry as u128;
        out.push(t as u64);
        carry = (t >> 64) as u64;
    }
    if carry != 0 {
        out.push(carry);
    }
    out
}

/// Naive subtraction (a >= b required).
pub fn sub_ref(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0u64;
    for (i, &ai) in a.iter().enumerate() {
        let x = ai as u128;
        let y = b.get(i).copied().unwrap_or(0) as u128 + borrow as u128;
        let t = x.wrapping_sub(y);
        out.push(t as u64);
        borrow = if x < y { 1 } else { 0 };
    }
    debug_assert_eq!(borrow, 0);
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// Naive division by repeated shifted subtraction: O(n^2) bit steps,
/// obviously correct.
pub fn divmod_ref(u: &[Limb], v: &[Limb]) -> (Vec<Limb>, Vec<Limb>) {
    assert!(!v.is_empty());
    let a = crate::biguint::BigUInt::from_limbs(u.to_vec());
    if crate::algos::add::cmp_slices(u, v) == std::cmp::Ordering::Less {
        return (Vec::new(), u.to_vec());
    }
    let bit_len = a.bit_length();
    let mut q = vec![0u64; bit_len.div_ceil(64)];
    let mut rem = crate::biguint::BigUInt::zero();
    for bit in (0..bit_len).rev() {
        rem = rem.shl(1);
        if a.bit(bit) {
            rem = rem.add(&crate::biguint::BigUInt::one());
        }
        if crate::algos::add::cmp_slices(rem.limbs(), v) != std::cmp::Ordering::Less {
            rem =
                crate::biguint::BigUInt::from_limbs(crate::algos::add::sub_slices(rem.limbs(), v));
            q[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    while q.last() == Some(&0) {
        q.pop();
    }
    (q, rem.into_limbs())
}

/// Naive decimal formatting: repeated division by 10^19 (single limb).
pub fn to_decimal_ref(u: &[Limb]) -> String {
    if u.is_empty() {
        return "0".to_string();
    }
    let mut chunks: Vec<u64> = Vec::new();
    let mut cur = u.to_vec();
    while !cur.is_empty() {
        let (q, r) = crate::algos::div::schoolbook::divmod_limb(&cur, crate::limb::DEC_CHUNK_BASE);
        chunks.push(r);
        cur = q;
    }
    let mut s = String::new();
    for (i, c) in chunks.iter().enumerate().rev() {
        if i == chunks.len() - 1 {
            s.push_str(&c.to_string());
        } else {
            s.push_str(&format!("{c:019}"));
        }
    }
    s
}

/// Naive decimal parsing: groups digits from the right into 19-digit
/// chunks, then multiply-accumulates from the most significant chunk.
pub fn from_decimal_ref(s: &str) -> Vec<Limb> {
    assert!(!s.is_empty());
    let base = crate::biguint::BigUInt::from(crate::limb::DEC_CHUNK_BASE);
    let mut chunks: Vec<u64> = Vec::new(); // least-significant first
    let mut idx = s.len();
    while idx > 0 {
        let start = idx.saturating_sub(crate::limb::DEC_CHUNK_DIGITS);
        chunks.push(s[start..idx].parse().unwrap());
        idx = start;
    }
    let mut acc = crate::biguint::BigUInt::zero();
    for &v in chunks.iter().rev() {
        acc = acc.mul(&base).add(&crate::biguint::BigUInt::from(v));
    }
    acc.into_limbs()
}
