//! x86-64 SIMD kernels: AVX2 and AVX-512F, runtime-dispatched with a
//! scalar fallback (ADR-0002 scope: add/sub/cmp vectorized; multiply and
//! NTT kernels stay scalar/parallel).
//!
//! Chunked add/sub via the SWAR pattern: lanes are summed without carries,
//! per-lane overflow flags are computed with an unsigned-greater-than
//! emulation, and a short serial fixup folds within-chunk carries plus the
//! incoming chunk carry. Results are bit-identical to [`super::scalar`]
//! by construction and by differential test (FR-3.3, S-7).

#[cfg(target_arch = "x86_64")]
pub mod imp {
    use super::super::{cpu_features, scalar};
    use crate::limb::{adc, sbb, Limb};
    use core::arch::x86_64::*;

    const LANES_AVX2: usize = 4;
    #[cfg(feature = "simd-avx512")]
    const LANES_AVX512: usize = 8;

    /// AVX2 kernel: adds one 4-limb chunk of `a + b (+ carry_in)` into the
    /// first 4 slots of `out`, returns the chunk carry-out.
    ///
    /// # Safety
    /// Caller must guarantee AVX2 support (runtime feature detection), and
    /// `a`, `b` must have at least 4 readable limbs, `out` at least 4
    /// writable limbs.
    #[target_feature(enable = "avx2")]
    pub unsafe fn add4_avx2(a: &[Limb], b: &[Limb], out: &mut [Limb], carry_in: Limb) -> Limb {
        // SAFETY: runtime AVX2 support is guaranteed by the caller; loads
        // read 32 bytes from each slice, the store writes 32 bytes.
        let va = _mm256_loadu_si256(a.as_ptr().cast::<__m256i>());
        let vb = _mm256_loadu_si256(b.as_ptr().cast::<__m256i>());
        let sum = _mm256_add_epi64(va, vb);
        // Per-lane unsigned overflow: a > sum (unsigned), emulated with the
        // xor-0x8000.. signed-compare trick because cmpgt is signed.
        let min = _mm256_set1_epi64x(i64::MIN);
        let ov = _mm256_cmpgt_epi64(_mm256_xor_si256(va, min), _mm256_xor_si256(sum, min));
        _mm256_storeu_si256(out.as_mut_ptr().cast::<__m256i>(), sum);
        let mask = _mm256_movemask_epi8(ov) as u32;
        fold_overflows(&mut out[..LANES_AVX2], carry_in, |lane| {
            ((mask >> (8 * lane)) & 0xFF) == 0xFF
        })
    }

    /// AVX2 kernel: `out[0..4] = a - b (- borrow_in)`, returns the chunk
    /// borrow-out.
    ///
    /// # Safety
    /// Same contract as [`add4_avx2`].
    #[target_feature(enable = "avx2")]
    pub unsafe fn sub4_avx2(a: &[Limb], b: &[Limb], out: &mut [Limb], borrow_in: Limb) -> Limb {
        // SAFETY: runtime AVX2 support guaranteed by the caller; 32-byte
        // loads/stores within the documented slice bounds.
        let va = _mm256_loadu_si256(a.as_ptr().cast::<__m256i>());
        let vb = _mm256_loadu_si256(b.as_ptr().cast::<__m256i>());
        let diff = _mm256_sub_epi64(va, vb);
        // Per-lane unsigned borrow: diff > a (unsigned) means lane borrowed.
        let min = _mm256_set1_epi64x(i64::MIN);
        let br = _mm256_cmpgt_epi64(_mm256_xor_si256(diff, min), _mm256_xor_si256(va, min));
        _mm256_storeu_si256(out.as_mut_ptr().cast::<__m256i>(), diff);
        let mask = _mm256_movemask_epi8(br) as u32;
        fold_borrows(&mut out[..LANES_AVX2], borrow_in, |lane| {
            ((mask >> (8 * lane)) & 0xFF) == 0xFF
        })
    }

    /// AVX-512F kernel: one 8-limb chunk add.
    ///
    /// Compiled only with the `simd-avx512` feature because the AVX-512
    /// intrinsics stabilized after the crate MSRV (ADR-0002).
    ///
    /// # Safety
    /// Caller must guarantee AVX-512F support and slice bounds with 8
    /// limbs per chunk.
    #[cfg(feature = "simd-avx512")]
    #[target_feature(enable = "avx512f")]
    // The feature gate above is the MSRV mechanism: this function compiles
    // only where the AVX-512 intrinsics exist (Rust >= 1.89).
    #[allow(clippy::incompatible_msrv)]
    pub unsafe fn add8_avx512(a: &[Limb], b: &[Limb], out: &mut [Limb], carry_in: Limb) -> Limb {
        // SAFETY: runtime AVX-512F support guaranteed by the caller; 64-byte
        // loads/stores stay within the documented slice bounds.
        let va = _mm512_loadu_si512(a.as_ptr().cast::<__m512i>());
        let vb = _mm512_loadu_si512(b.as_ptr().cast::<__m512i>());
        let sum = _mm512_add_epi64(va, vb);
        let min = _mm512_set1_epi64(i64::MIN);
        let ov = _mm512_cmpgt_epi64_mask(_mm512_xor_si512(va, min), _mm512_xor_si512(sum, min));
        _mm512_storeu_si512(out.as_mut_ptr().cast::<__m512i>(), sum);
        fold_overflows(&mut out[..LANES_AVX512], carry_in, |lane| {
            (ov >> lane) & 1 == 1
        })
    }

    /// Fold per-lane overflow flags and an incoming carry into the chunk:
    /// lane i+1 absorbs a +1 for every overflow at lane i (with wrap
    /// propagation). Returns the chunk carry-out.
    fn fold_overflows(out: &mut [Limb], carry_in: Limb, ovf: impl Fn(usize) -> bool) -> Limb {
        let mut incoming = carry_in;
        for (lane, slot) in out.iter_mut().enumerate() {
            let (v, wrapped) = adc(*slot, incoming, 0);
            *slot = v;
            incoming = if ovf(lane) || wrapped == 1 { 1 } else { 0 };
        }
        incoming
    }

    /// Fold per-lane borrow flags and an incoming borrow: lane i+1 absorbs
    /// a -1. Returns the chunk borrow-out.
    fn fold_borrows(out: &mut [Limb], borrow_in: Limb, br: impl Fn(usize) -> bool) -> Limb {
        let mut incoming = borrow_in;
        for (lane, slot) in out.iter_mut().enumerate() {
            let (v, borrowed) = sbb(*slot, incoming, 0);
            *slot = v;
            incoming = if br(lane) || borrowed == 1 { 1 } else { 0 };
        }
        incoming
    }

    /// Runtime-dispatched chunked add: AVX-512F if present, else AVX2,
    /// else scalar. Tail limbs use the scalar chain.
    pub fn add(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
        let feats = cpu_features();
        #[cfg(feature = "simd-avx512")]
        if feats.avx512f {
            return chunked(a, b, LANES_AVX512, |a, b, out, ci| unsafe {
                // SAFETY: avx512f detected above; chunks have 8 limbs.
                add8_avx512(a, b, out, ci)
            });
        }
        if feats.avx2 {
            return chunked(a, b, LANES_AVX2, |a, b, out, ci| unsafe {
                // SAFETY: avx2 detected above; chunks have 4 limbs.
                add4_avx2(a, b, out, ci)
            });
        }
        scalar::add(a, b)
    }

    /// Runtime-dispatched chunked subtract (a >= b).
    pub fn sub(a: &[Limb], b: &[Limb]) -> Vec<Limb> {
        let feats = cpu_features();
        if feats.avx2 {
            return chunked_sub(a, b, LANES_AVX2, |a, b, out, bi| unsafe {
                // SAFETY: avx2 detected above; chunks have 4 limbs.
                sub4_avx2(a, b, out, bi)
            });
        }
        scalar::sub(a, b)
    }

    type ChunkKernel = fn(&[Limb], &[Limb], &mut [Limb], Limb) -> Limb;

    fn chunked(a: &[Limb], b: &[Limb], lanes: usize, kernel: ChunkKernel) -> Vec<Limb> {
        let n = a.len().max(b.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u64;
        let mut start = 0;
        while start + lanes <= n {
            let mut ain = [0u64; 8];
            let mut bin = [0u64; 8];
            ain[..lanes].copy_from_slice(&zero_pad(a, start, lanes));
            bin[..lanes].copy_from_slice(&zero_pad(b, start, lanes));
            let mut chunk_out = [0u64; 8];
            carry = kernel(&ain, &bin, &mut chunk_out, carry);
            out.extend_from_slice(&chunk_out[..lanes]);
            start += lanes;
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
        out
    }

    fn chunked_sub(a: &[Limb], b: &[Limb], lanes: usize, kernel: ChunkKernel) -> Vec<Limb> {
        let n = a.len().max(b.len());
        let mut out = Vec::with_capacity(n);
        let mut borrow = 0u64;
        let mut start = 0;
        while start + lanes <= n {
            let mut ain = [0u64; 8];
            let mut bin = [0u64; 8];
            ain[..lanes].copy_from_slice(&zero_pad(a, start, lanes));
            bin[..lanes].copy_from_slice(&zero_pad(b, start, lanes));
            let mut chunk_out = [0u64; 8];
            borrow = kernel(&ain, &bin, &mut chunk_out, borrow);
            out.extend_from_slice(&chunk_out[..lanes]);
            start += lanes;
        }
        for i in start..n {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            let (d, br) = sbb(x, y, borrow);
            out.push(d);
            borrow = br;
        }
        debug_assert_eq!(borrow, 0, "simd sub requires a >= b");
        while out.last() == Some(&0) {
            out.pop();
        }
        out
    }

    fn zero_pad(v: &[Limb], start: usize, lanes: usize) -> Vec<Limb> {
        (0..lanes)
            .map(|i| v.get(start + i).copied().unwrap_or(0))
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dispatched_add_matches_scalar() {
            let mut s: u64 = 0xFEED_51D1;
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
                let want = scalar::add(&a, &b);
                assert_eq!(add(&a, &b), want, "dispatched add at len {len}");
            }
        }

        #[test]
        fn dispatched_sub_matches_scalar() {
            if !cpu_features().avx2 {
                return; // scalar fallback already covered by scalar tests
            }
            let mut s: u64 = 0xBEEF_D00D;
            for len in 1..70usize {
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
                if crate::algos::add::cmp_slices(&a, &b) == std::cmp::Ordering::Less {
                    continue;
                }
                let want = scalar::sub(&a, &b);
                assert_eq!(sub(&a, &b), want, "dispatched sub at len {len}");
            }
        }

        #[test]
        fn avx2_kernel_direct_when_available() {
            if !cpu_features().avx2 {
                return;
            }
            // Carry-cascade: all-ones plus 1.
            let a = [u64::MAX, u64::MAX, u64::MAX, u64::MAX];
            let b = [1, 0, 0, 0];
            let mut out = [0u64; 4];
            let co = unsafe {
                // SAFETY: avx2 support checked above; slices have 4 limbs.
                add4_avx2(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [0, 0, 0, 0]);
            assert_eq!(co, 1);
            // Carry-in into a MAX lane must ripple.
            let a = [u64::MAX, 0, 0, 0];
            let b = [0, 0, 0, 0];
            let mut out = [0u64; 4];
            let co = unsafe {
                // SAFETY: avx2 support checked above.
                add4_avx2(&a, &b, &mut out, 1)
            };
            assert_eq!(out, [0, 1, 0, 0]);
            assert_eq!(co, 0);
            // Same-lane overflow plus borrow fold.
            let a = [u64::MAX, 0, 0, 0];
            let b = [u64::MAX, 0, 0, 0];
            let mut out = [0u64; 4];
            let co = unsafe {
                // SAFETY: avx2 support checked above.
                add4_avx2(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [u64::MAX - 1, 1, 0, 0]);
            assert_eq!(co, 0);
        }

        #[test]
        #[cfg(feature = "simd-avx512")]
        fn avx512_kernel_direct_when_available() {
            if !cpu_features().avx512f {
                return;
            }
            let a = [u64::MAX; 8];
            let b = [1, 0, 0, 0, 0, 0, 0, 0];
            let mut out = [0u64; 8];
            let co = unsafe {
                // SAFETY: avx512f support checked above; 8 limbs per slice.
                add8_avx512(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [0u64; 8]);
            assert_eq!(co, 1);
        }

        #[test]
        fn avx2_sub_kernel_direct_when_available() {
            if !cpu_features().avx2 {
                return;
            }
            // a - b with borrow ripple: [0,1,0,0] - [1,0,0,0]
            let a = [0, 1, 0, 0];
            let b = [1, 0, 0, 0];
            let mut out = [0u64; 4];
            let bo = unsafe {
                // SAFETY: avx2 support checked above.
                sub4_avx2(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [u64::MAX, 0, 0, 0]);
            assert_eq!(bo, 0);
            // Internal borrow ripple: 2^65 - (2^64 + 1) = 2^64 - 1.
            let a = [0, 2, 0, 0];
            let b = [1, 1, 0, 0];
            let mut out = [0u64; 4];
            let bo = unsafe {
                // SAFETY: avx2 support checked above.
                sub4_avx2(&a, &b, &mut out, 0)
            };
            assert_eq!(out, [u64::MAX, 0, 0, 0]);
            assert_eq!(bo, 0);
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub use imp::*;
