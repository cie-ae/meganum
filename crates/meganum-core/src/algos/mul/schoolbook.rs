//! Schoolbook multiplication, O(n^2); the correctness baseline for every
//! faster multiply.

use crate::limb::{mul_add_carry, Limb};

/// Multiply canonical limb slices `a * b`; returns a canonical product.
pub fn mul(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut res = vec![0u64; a.len() + b.len()];
    for (i, &ai) in a.iter().enumerate() {
        if ai == 0 {
            continue;
        }
        let mut carry = 0u64;
        for (j, &bj) in b.iter().enumerate() {
            let (low, high) = mul_add_carry(ai, bj, res[i + j], carry);
            res[i + j] = low;
            carry = high;
        }
        // Propagate the final row carry; it cannot overflow past the top.
        let mut k = i + b.len();
        while carry != 0 {
            let t = res[k] as u128 + carry as u128;
            res[k] = t as u64;
            carry = (t >> 64) as u64;
            k += 1;
        }
    }
    while res.last() == Some(&0) {
        res.pop();
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_zero() {
        assert!(mul(&[], &[1]).is_empty());
        assert!(mul(&[1], &[]).is_empty());
        assert!(mul(&[0, 0], &[5]).is_empty());
        assert_eq!(mul(&[3], &[4]), vec![12]);
        assert_eq!(mul(&[u64::MAX], &[2]), vec![u64::MAX - 1, 1]);
    }

    #[test]
    fn square_of_all_ones() {
        let ones = vec![u64::MAX, u64::MAX, u64::MAX];
        let p = mul(&ones, &ones);
        // (2^192 - 1)^2 = 2^384 - 2^193 + 1: bits 194..=383 set plus bit 0.
        let want = vec![1u64, 0, 0, u64::MAX - 1, u64::MAX, u64::MAX];
        assert_eq!(p, want);
    }

    #[test]
    fn associativity_spot() {
        let a = [7, 9, 11, 13];
        let b = [u64::MAX, 5];
        let c = [3, 0, 1];
        let ab = mul(&a, &b);
        let bc = mul(&b, &c);
        let left = mul(&ab, &c);
        let right = mul(&a, &bc);
        assert_eq!(left, right);
        assert_eq!(mul(&a, &b), mul(&b, &a));
    }
}
