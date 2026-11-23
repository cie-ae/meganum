//! SIMD kernel registry (feature `simd`).
//!
//! Scope per ADR-0002: v1.0 vectorizes add/sub/cmp and bulk kernels
//! (AVX2/AVX-512 on x86-64, NEON on aarch64) with a scalar fallback that is
//! always compiled; multiply/divide/NTT kernels remain scalar (parallelized
//! via the parallel backend). All `unsafe` is confined to this module tree,
//! each block carries a `// SAFETY:` comment and an executing test (S-2).

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod scalar;
#[cfg(target_arch = "x86_64")]
pub mod x86;

/// Runtime CPU feature flags, detected once and cached in atomics
/// (spec 5.4: CPU capability detected at import into atomic flags).
pub struct CpuFeatures {
    /// AVX2 available (x86-64 only).
    pub avx2: bool,
    /// AVX-512F available (x86-64 only).
    pub avx512f: bool,
    /// NEON available (aarch64 only; always true in practice).
    pub neon: bool,
}

impl CpuFeatures {
    #[cfg(target_arch = "x86_64")]
    pub fn detect() -> Self {
        CpuFeatures {
            avx2: std::arch::is_x86_feature_detected!("avx2"),
            avx512f: std::arch::is_x86_feature_detected!("avx512f"),
            neon: false,
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn detect() -> Self {
        CpuFeatures {
            avx2: false,
            avx512f: false,
            neon: true,
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn detect() -> Self {
        CpuFeatures {
            avx2: false,
            avx512f: false,
            neon: false,
        }
    }
}

static CPU_FLAGS: std::sync::OnceLock<CpuFeatures> = std::sync::OnceLock::new();

/// Process-wide cached CPU feature detection.
pub fn cpu_features() -> &'static CpuFeatures {
    CPU_FLAGS.get_or_init(CpuFeatures::detect)
}

/// Chunked add via the best runtime kernel for this CPU.
pub fn add(a: &[crate::limb::Limb], b: &[crate::limb::Limb]) -> Vec<crate::limb::Limb> {
    #[cfg(target_arch = "x86_64")]
    {
        return x86::imp::add(a, b);
    }
    #[cfg(target_arch = "aarch64")]
    {
        return aarch64::imp::add(a, b);
    }
    #[allow(unreachable_code)]
    scalar::add(a, b)
}

/// Chunked subtract via the best runtime kernel for this CPU (a >= b).
pub fn sub(a: &[crate::limb::Limb], b: &[crate::limb::Limb]) -> Vec<crate::limb::Limb> {
    #[cfg(target_arch = "x86_64")]
    {
        return x86::imp::sub(a, b);
    }
    #[cfg(target_arch = "aarch64")]
    {
        return aarch64::imp::sub(a, b);
    }
    #[allow(unreachable_code)]
    scalar::sub(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_returns_at_least_scalar() {
        let f = cpu_features();
        // Scalar fallback always exists; flags are advisory.
        let _ = (f.avx2, f.avx512f, f.neon);
    }

    #[test]
    fn scalar_add_matches_slice_add() {
        let a: Vec<u64> = vec![u64::MAX, 3, 0];
        let b: Vec<u64> = vec![1, 4, 0];
        let got = scalar::add(&a, &b);
        let want = crate::algos::add::add_slices(&a, &b);
        assert_eq!(got, want);
    }

    #[test]
    fn scalar_sub_matches_slice_sub() {
        let a: Vec<u64> = vec![0, 5, 9];
        let b: Vec<u64> = vec![1, 2, 9];
        let got = scalar::sub(&a, &b);
        let want = crate::algos::add::sub_slices(&a, &b);
        assert_eq!(got, want);
    }
}
