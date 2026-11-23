//! Build facts exposed through `meganum.about()` (spec section 8.1).

use pyo3::prelude::*;
use std::collections::BTreeMap;

/// Compile-time set of enabled features, injected by maturin/cargo.
fn compiled_features() -> Vec<&'static str> {
    let mut v = Vec::new();
    if cfg!(feature = "parallel") {
        v.push("parallel");
    }
    if cfg!(feature = "simd") {
        v.push("simd");
    }
    v
}

/// Runtime-detectable SIMD paths on this machine.
fn simd_paths() -> Vec<&'static str> {
    let mut v = vec!["scalar"];
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            v.push("avx2");
        }
        if std::arch::is_x86_feature_detected!("avx512f") {
            v.push("avx512f");
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            v.push("neon");
        }
    }
    v
}

/// Report build facts: versions, compiled features, SIMD paths.
#[pyfunction]
pub fn about() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert(
        "version".to_string(),
        meganum_core::CORE_VERSION.to_string(),
    );
    m.insert(
        "rustc".to_string(),
        option_env!("MEGANUM_RUSTC_VERSION")
            .unwrap_or("unknown")
            .to_string(),
    );
    m.insert("features".to_string(), compiled_features().join(","));
    m.insert("simd".to_string(), simd_paths().join(","));
    m.insert(
        "workers".to_string(),
        std::thread::available_parallelism()
            .map(|n| n.get().to_string())
            .unwrap_or_else(|_| "1".to_string()),
    );
    m
}
