//! aarch64 SIMD kernels (NEON), runtime-dispatched with a scalar fallback.
//!
//! Same SWAR chunked pattern as the x86-64 module; NEON `vcltq_u64` gives a
//! direct unsigned compare so no xor trick is needed. Bit-identical to
//! [`super::scalar`] by construction and by differential test.

#[cfg(target_arch = "aarch64")]
pub mod imp {
    use super::super::{cpu_features, scalar};
    use crate::limb::{adc, Limb};
    use core::arch::aarch64::*;

    /// NEON kernel: one 2-lane chunk add into `out`, returns carry-out.
    ///
    /// # Safety
    /// Caller must guarantee NEON support (universal on aarch64) and slice
    /// bounds of at least 2 limbs per slice.
    #[target_feature(enable = "neon")]
    pub unsafe fn add2_neon(a: &[Limb], b: &[Limb], out: &mut [Limb], carry_in: Limb) -> Limb {
        // SAFETY: NEON is available on all aarch64 targets we support; the
        // 16-byte loads/stores stay within the documented slice bounds.
        let va = vld1q_u64(a.as_ptr());
        let vb = vld1q_u64(b.as_ptr());
        let sum = vaddq_u64(va, vb);
        // Per-lane unsigned overflow: sum < a (a wrapped sum is always
        // smaller than its addend). vcltq_u64(x, y) is lane-wise x < y.
        let ov = vcltq_u64(sum, va);
        vst1q_u64(out.as_mut_ptr(), sum);
        let lo = vgetq_lane_u64(ov, 0);
        let hi = vgetq_lane_u64(ov, 1);
        fold(out, carry_in, lo != 0, hi != 0)
    }

    fn fold(out: &mut [Limb], carry_in: Limb, ov0: bool, ov1: bool) -> Limb {
        let mut incoming = carry_in;
        for (lane, ovf) in [ov0, ov1].into_iter().enumerate() {
            let (v, wrapped) = adc(out[lane], incoming, 0);
            out[lane] = v;
            incoming = if ovf || wrapped == 1 { 1 } else { 0 };
        }
        incoming
    }

    /// Runtime-dispatched chunked add; scalar tail beyond 2-lane chunks.
    pub fn add(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
        if cpu_features().neon {
            let n = a.len().max(b.len());
            let mut out = Vec::with_capacity(n + 1);
            let mut carry = 0u64;
            let mut start = 0;
            while start + 2 <= n {
                let mut ain = [0u64; 2];
                let mut bin = [0u64; 2];
                for i in 0..2 {
                    ain[i] = a.get(start + i).copied().unwrap_or(0);
                    bin[i] = b.get(start + i).copied().unwrap_or(0);
                }
                let mut chunk_out = [0u64; 2];
                let co = unsafe {
                    // SAFETY: NEON universal on aarch64; 2 limbs per chunk.
                    add2_neon(&ain, &bin, &mut chunk_out, carry)
                };
                out.extend_from_slice(&chunk_out);
                carry = co;
                start += 2;
            }
            for i in start..n {
                let x = a.get(i).copied().unwrap_or(0);
                let y = b.get(i).copied().unwrap_or(0);
                let (s, c) = adc(x, y, carry);
                out.push(s);
                carry = c;
            }
            if carry != 0 {
                out.push(carry);
            }
            while out.last() == Some(&0) {
                out.pop();
            }
            return out;
        }
        scalar::add(a, b)
    }

    pub fn sub(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
        scalar::sub(a, b)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dispatched_add_matches_scalar() {
            let mut s: u64 = 0x051D_EA64;
            for len in 0..40usize {
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
                let want = scalar::add(&a, &b);
                assert_eq!(add(&a, &b), want, "len {len}");
            }
        }

        #[test]
        fn neon_kernel_direct() {
            // Carry-cascade: all-ones plus 1.
            let a = [u64::MAX, u64::MAX];
            let b = [1, 0];
            let mut out = [0u64; 2];
            let co = unsafe {
                // SAFETY: NEON universal on aarch64; 2 limbs per slice.
                add2_neon(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [0, 0]);
            assert_eq!(co, 1);
            // Carry-in into a MAX lane must ripple.
            let a = [u64::MAX, 0];
            let b = [0, 0];
            let mut out = [0u64; 2];
            let co = unsafe {
                // SAFETY: NEON universal on aarch64; 2 limbs per slice.
                add2_neon(&a, &b, &mut out, 1)
            };
            assert_eq!(out, [0, 1]);
            assert_eq!(co, 0);
            // Same-lane overflow without a chunk-boundary carry: the lane-1
            // phantom-carry regression (inverted overflow flag).
            let a = [7, u64::MAX];
            let b = [2, 1];
            let mut out = [0u64; 2];
            let co = unsafe {
                // SAFETY: NEON universal on aarch64; 2 limbs per slice.
                add2_neon(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [9, 0]);
            assert_eq!(co, 1);
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub use imp::*;
