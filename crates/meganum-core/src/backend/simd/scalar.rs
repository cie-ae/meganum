//! Scalar fallback kernels: always compiled, bit-identical to the
//! vectorized paths, and the semantic reference for them.

use crate::limb::{adc, Limb};

/// Scalar add of two canonical limb slices.
pub fn add(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    crate::algos::add::add_slices(a, b)
}

/// Scalar subtract of two canonical limb slices (a >= b).
pub fn sub(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
    crate::algos::add::sub_slices(a, b)
}

/// Scalar reference of the chunked add kernel: identical semantics to the
/// SIMD chunked kernel - each chunk is summed with an intra-chunk carry
/// chain, and the chunk carry-out feeds the next chunk.
pub fn add_chunked(a: &[Limb], b: &[Limb], chunk: usize) -> Vec<Limb> {
    let n = a.len().max(b.len());
    let chunk = chunk.max(1);
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0u64;
    let mut start = 0;
    while start < n {
        let end = (start + chunk).min(n);
        for i in start..end {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            let (s, c) = adc(x, y, carry);
            out.push(s);
            carry = c;
        }
        start = end;
    }
    if carry != 0 {
        out.push(carry);
    }
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// Scalar reference of the chunked subtract kernel (a >= b).
pub fn sub_chunked(a: &[Limb], b: &[Limb], chunk: usize) -> Vec<Limb> {
    let n = a.len().max(b.len());
    let chunk = chunk.max(1);
    let mut out = Vec::with_capacity(n);
    let mut borrow = 0u64;
    let mut start = 0;
    while start < n {
        let end = (start + chunk).min(n);
        for i in start..end {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            let (d, br) = crate::limb::sbb(x, y, borrow);
            out.push(d);
            borrow = br;
        }
        start = end;
    }
    debug_assert_eq!(borrow, 0, "sub_chunked requires a >= b");
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_equals_plain_random() {
        let mut s: u64 = 0x00C0_FFEE_C0DE;
        for len in 0..70usize {
            let a: Vec<u64> = (0..len)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    s
                })
                .collect();
            let b: Vec<u64> = (0..len / 2 + 1)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    s
                })
                .collect();
            for chunk in [1usize, 2, 3, 4, 8, 17] {
                assert_eq!(
                    add_chunked(&a, &b, chunk),
                    add(&a, &b),
                    "add len {len} chunk {chunk}"
                );
            }
        }
    }
}
