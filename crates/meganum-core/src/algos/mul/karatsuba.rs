//! Karatsuba multiplication, O(n^1.585), with a pooled scratch arena.

use crate::algos::add::{add_slices, sub_slices};
use crate::algos::mul::schoolbook::mul as schoolbook_mul;
use crate::algos::Scratch;
use crate::limb::Limb;

/// Karatsuba recursion cutoff (below this, schoolbook wins).
const BASECASE_LIMBS: usize = 32;

/// Multiply canonical slices with Karatsuba splitting.
pub fn mul(a: &[Limb], b: &[Limb], scratch: &mut Scratch) -> Vec<Limb> {
    let n = a.len().max(b.len());
    if n <= BASECASE_LIMBS || a.is_empty() || b.is_empty() {
        return schoolbook_mul(a, b);
    }
    let m = n.div_ceil(2);

    let (a_lo, a_hi) = split(a, m);
    let (b_lo, b_hi) = split(b, m);

    let z0 = mul(a_lo, b_lo, scratch);
    let z2 = mul(a_hi, b_hi, scratch);

    let sa = add_slices(a_lo, a_hi);
    let sb = add_slices(b_lo, b_hi);
    let z1_full = mul(&sa, &sb, scratch);
    let z1 = sub_slices(&sub_slices(&z1_full, &z0), &z2);
    scratch.put(z1_full);

    // result = z2 * 2^(128m) + z1 * 2^(64m) + z0
    let mut out = vec![0u64; a.len() + b.len()];
    copy_into(&mut out, &z0, 0);
    add_into(&mut out, &z1, m);
    add_into(&mut out, &z2, 2 * m);
    while out.last() == Some(&0) {
        out.pop();
    }
    out
}

/// Split a canonical slice at limb `m` into (low, high) parts; the high part
/// is trimmed (may be empty).
fn split(a: &[Limb], m: usize) -> (&[Limb], &[Limb]) {
    if a.len() <= m {
        (a, &[])
    } else {
        (&a[..m], &a[m..])
    }
}

/// Write `src` starting at limb offset `off` (overwrite, no carry assumed:
/// used only for z0 whose length fits).
fn copy_into(out: &mut [Limb], src: &[Limb], off: usize) {
    out[off..off + src.len()].copy_from_slice(src);
}

/// Add `src` into `out` starting at limb offset `off`, propagating carries.
fn add_into(out: &mut [Limb], src: &[Limb], off: usize) {
    let mut carry = 0u64;
    for (i, &s) in src.iter().enumerate() {
        let t = out[off + i] as u128 + s as u128 + carry as u128;
        out[off + i] = t as u64;
        carry = (t >> 64) as u64;
    }
    let mut k = off + src.len();
    while carry != 0 {
        let t = out[k] as u128 + carry as u128;
        out[k] = t as u64;
        carry = (t >> 64) as u64;
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algos::mul::schoolbook::mul as ref_mul;

    fn compare_sizes(na: usize, nb: usize) {
        let mut rng: u64 = 0x243F6A8885A308D3;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let a: Vec<Limb> = (0..na).map(|_| next()).collect();
        let b: Vec<Limb> = (0..nb).map(|_| next()).collect();
        let mut scratch = Scratch::new();
        let got = mul(&a, &b, &mut scratch);
        let want = ref_mul(&a, &b);
        assert_eq!(got, want, "karatsuba vs schoolbook at ({na}, {nb}) limbs");
    }

    #[test]
    fn matches_schoolbook_across_sizes() {
        for n in [30usize, 31, 32, 33, 40, 41, 64, 65, 100, 128, 257, 300] {
            compare_sizes(n, n);
            compare_sizes(n, n / 2 + 3);
            compare_sizes(n / 3 + 1, n);
        }
    }

    #[test]
    fn unbalanced_edge_shapes() {
        compare_sizes(41, 1);
        compare_sizes(1, 41);
        compare_sizes(64, 2);
    }
}
