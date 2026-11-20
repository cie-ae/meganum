//! Addition and subtraction over limb slices (schoolbook carry chains).
//!
//! These are the algorithm-layer primitives used by every backend; the SIMD
//! backend implements chunked kernels producing bit-identical results.

use crate::limb::{adc, sbb, Limb};

/// `a + b` for canonical slices.
pub fn add_slices(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut out = Vec::with_capacity(long.len() + 1);
    let mut carry = 0u64;
    for i in 0..short.len() {
        let (s, c) = adc(long[i], short[i], carry);
        out.push(s);
        carry = c;
    }
    for &l in &long[short.len()..] {
        let (s, c) = adc(l, 0, carry);
        out.push(s);
        carry = c;
    }
    if carry != 0 {
        out.push(carry);
    }
    out
}

/// `a - b` for canonical slices with `a >= b` (debug-checked).
/// Output is canonical (trailing zero limbs trimmed).
pub fn sub_slices(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    debug_assert!(a.len() >= b.len(), "sub_slices requires a.len() >= b.len()");
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0u64;
    for (i, &ai) in a.iter().enumerate() {
        let bi = b.get(i).copied().unwrap_or(0);
        let (d, br) = sbb(ai, bi, borrow);
        out.push(d);
        borrow = br;
    }
    debug_assert_eq!(borrow, 0, "sub_slices requires a >= b");
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// Compare canonical slices.
pub fn cmp_slices(a: &[Limb], b: &[Limb]) -> std::cmp::Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for i in (0..a.len()).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_carry_chains() {
        assert!(add_slices(&[], &[]).is_empty());
        assert_eq!(add_slices(&[], &[5]), vec![5]);
        assert_eq!(add_slices(&[u64::MAX, u64::MAX], &[1]), vec![0, 0, 1]);
        assert_eq!(add_slices(&[1, 2, 3], &[1, 2, 3]), vec![2, 4, 6]);
    }

    #[test]
    fn sub_borrow_chains() {
        assert_eq!(sub_slices(&[5], &[]), vec![5]);
        assert_eq!(sub_slices(&[0, 1], &[1]), vec![u64::MAX]);
        assert_eq!(sub_slices(&[2, 4, 6], &[1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn cmp_slice_order() {
        assert_eq!(cmp_slices(&[1, 2], &[1, 2]), std::cmp::Ordering::Equal);
        assert_eq!(cmp_slices(&[1, 2], &[3]), std::cmp::Ordering::Greater);
        assert_eq!(cmp_slices(&[], &[1]), std::cmp::Ordering::Less);
    }
}
