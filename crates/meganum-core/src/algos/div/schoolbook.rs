//! Schoolbook long division (Knuth Algorithm D, TAOCP 4.3.1).
//!
//! The correctness baseline for every faster division; also the fast path
//! for small quotients and single-limb divisors.

use crate::limb::{div_wide, Limb, LIMB_BITS};

/// Divide by a single limb; returns (quotient, remainder).
pub fn divmod_limb(u: &[Limb], d: Limb) -> (Vec<Limb>, Limb) {
    debug_assert!(d != 0);
    let mut q = vec![0u64; u.len()];
    let mut rem = 0u64;
    for i in (0..u.len()).rev() {
        let (qt, rr) = div_wide(rem, u[i], d);
        q[i] = qt;
        rem = rr;
    }
    while q.last() == Some(&0) {
        q.pop();
    }
    (q, rem)
}

/// Full Knuth-D division of canonical magnitudes; `v` canonical and nonzero.
/// Returns (quotient, remainder) with 0 <= r < v.
pub fn divmod(u: &[Limb], v: &[Limb]) -> (Vec<Limb>, Vec<Limb>) {
    debug_assert!(
        !v.is_empty() && *v.last().unwrap() != 0,
        "v must be canonical nonzero"
    );
    if v.len() == 1 {
        let (q, r) = divmod_limb(u, v[0]);
        return (q, if r == 0 { Vec::new() } else { vec![r] });
    }
    if crate::algos::add::cmp_slices(u, v) == std::cmp::Ordering::Less {
        return (Vec::new(), u.to_vec());
    }

    let m = v.len();
    let n = u.len();
    debug_assert!(n >= m);

    // Normalize: shift so the top limb of v has its MSB set.
    let shift = v[m - 1].leading_zeros();
    // vn keeps exactly m limbs (the shifted top limb cannot overflow into a
    // new limb because the top limb's leading zeros cover the shift).
    let mut vn = shl_slices(v, shift);
    vn.truncate(m);
    debug_assert_eq!(vn.len(), m);
    let mut un = shl_slices(u, shift); // n + 1 limbs

    let mut q = vec![0u64; n - m + 1];
    for j in (0..=n - m).rev() {
        // Estimate qhat = floor((un[j+m]*B + un[j+m-1]) / vn[m-1]).
        let numerator = ((un[j + m] as u128) << LIMB_BITS) | un[j + m - 1] as u128;
        let mut qhat = numerator / vn[m - 1] as u128;
        let mut rhat = numerator % vn[m - 1] as u128;

        // Refinement (Hacker's Delight divmnu): at most two corrections.
        loop {
            let too_big = qhat >= 1u128 << LIMB_BITS
                || qhat * vn[m - 2] as u128 > ((rhat << LIMB_BITS) | un[j + m - 2] as u128);
            if !too_big {
                break;
            }
            qhat -= 1;
            rhat += vn[m - 1] as u128;
            if rhat >= 1u128 << LIMB_BITS {
                break;
            }
        }
        let mut qhat_l = qhat as u64;

        // Multiply and subtract: un[j..=j+m] -= qhat * vn.
        let borrow = mul_sub_into(&mut un[j..=j + m], &vn, qhat_l);
        if borrow != 0 {
            // Add back once (probability ~2/B per Knuth).
            qhat_l -= 1;
            let mut carry = 0u64;
            for i in 0..m {
                let t = un[j + i] as u128 + vn[i] as u128 + carry as u128;
                un[j + i] = t as u64;
                carry = (t >> LIMB_BITS) as u64;
            }
            un[j + m] = un[j + m].wrapping_add(carry);
        }
        q[j] = qhat_l;
    }

    let mut rem = shr_slices(&un[..m], shift);
    while q.last() == Some(&0) {
        q.pop();
    }
    while rem.last() == Some(&0) {
        rem.pop();
    }
    (q, rem)
}

/// Shift a canonical slice left by `bits < 64`; returns `a.len() + 1` limbs
/// (the extra top limb may be zero).
pub fn shl_slices(a: &[Limb], bits: u32) -> Vec<Limb> {
    if bits == 0 {
        let mut v = a.to_vec();
        v.push(0);
        return v;
    }
    let mut out = Vec::with_capacity(a.len() + 1);
    let mut carry = 0u64;
    for &l in a {
        out.push((l << bits) | carry);
        carry = l >> (LIMB_BITS - bits);
    }
    out.push(carry);
    out
}

/// Shift right by `bits < 64` (floor), canonical output.
pub fn shr_slices(a: &[Limb], bits: u32) -> Vec<Limb> {
    if bits == 0 {
        return a.to_vec();
    }
    let mut out = Vec::with_capacity(a.len());
    for i in 0..a.len() {
        let lo = a[i] >> bits;
        let hi = if i + 1 < a.len() {
            a[i + 1] << (LIMB_BITS - bits)
        } else {
            0
        };
        out.push(lo | hi);
    }
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// `dst[0..=m] -= q * src[0..m]` in place (m = src.len()).
/// Returns 1 if the true result was negative (caller must add back).
fn mul_sub_into(dst: &mut [Limb], src: &[Limb], q: Limb) -> Limb {
    // Signed accumulator; invariant: 0 <= borrow < 2^64 + 2.
    let mut borrow: i128 = 0;
    for i in 0..src.len() {
        let p = q as u128 * src[i] as u128;
        let p_lo = (p & 0xFFFF_FFFF_FFFF_FFFF) as i128;
        let p_hi = (p >> 64) as i128;
        let t = dst[i] as i128 - p_lo - borrow;
        dst[i] = t as u64; // wrapping cast keeps the low 64 bits
        borrow = p_hi - (t >> 64);
    }
    let t = dst[src.len()] as i128 - borrow;
    dst[src.len()] = t as u64;
    if t < 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::mul::schoolbook::mul;

    fn check(u: &[Limb], v: &[Limb]) {
        let (q, r) = if v.len() == 1 {
            let (q, r) = divmod_limb(u, v[0]);
            (q, if r == 0 { Vec::new() } else { vec![r] })
        } else {
            divmod(u, v)
        };
        let prod = mul(&q, v);
        let sum = crate::algos::add::add_slices(&prod, &r);
        assert_eq!(sum, u.to_vec(), "q*v+r != u for u={u:?} v={v:?}");
        assert!(
            crate::algos::add::cmp_slices(&r, v) == std::cmp::Ordering::Less,
            "remainder out of range"
        );
    }

    #[test]
    fn basic_single_limb_divisor() {
        check(&[10, 0, 0, 1], &[3]);
        check(&[u64::MAX, u64::MAX], &[u64::MAX]);
        check(&[1], &[2]);
        check(&[0, 0, 1], &[5]);
    }

    #[test]
    fn multi_limb_cases() {
        check(&[1, 2, 3, 4, 5], &[7, 8]);
        check(&[u64::MAX, u64::MAX, u64::MAX, u64::MAX], &[u64::MAX, 1]);
        check(&[0xdeadbeef, 0xcafebabe, 1], &[2, 0, 1]);
        check(&[9, 0, 0, 0, 5], &[6, 0, 5]);
        check(&[123456789, 987654321, 555555555, 1], &[u64::MAX, 0, 1]);
        check(
            &[u64::MAX, 0, u64::MAX, 0, u64::MAX],
            &[u64::MAX, u64::MAX >> 3, 7],
        );
    }

    #[test]
    fn quotient_equals_one() {
        let v = vec![7, 8, 9];
        let u = vec![0, 7, 8, 9];
        check(&u, &v);
        let u = vec![7, 8, 9];
        check(&u, &v);
    }

    #[test]
    fn remainder_exactly_v_minus_one() {
        let v = vec![u64::MAX, 1];
        let u = vec![u64::MAX - 1, u64::MAX, 3];
        check(&u, &v);
    }

    #[test]
    fn normalization_edge_top_bit() {
        // Divisor already has MSB set (shift == 0).
        let v = vec![u64::MAX, u64::MAX];
        let u = vec![3, 4, 5, 6, 7];
        check(&u, &v);
    }

    #[test]
    fn shr_shl_slice_helpers() {
        assert_eq!(shr_slices(&[0, 2], 1), vec![0, 1]);
        // 2^64 << 63 = 2^127: limb 1 bit 63; the extra top limb stays zero.
        assert_eq!(shl_slices(&[0, 1], 63), vec![0, 1 << 63, 0]);
        assert_eq!(shr_slices(&[5], 0), vec![5]);
    }
}
