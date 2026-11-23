//! Criterion benchmarks for meganum-core primitives and algorithms.
//!
//! Methodology: see benches/METHODOLOGY.md (median of 100+ iterations after
//! 3 s warmup; p50 and p95 reported; same-runner-class comparisons only).

use criterion::{criterion_group, criterion_main, Criterion};
use meganum_core::limb::{adc, mul_wide, sbb};
use std::hint::black_box;

fn bench_limb_primitives(c: &mut Criterion) {
    c.bench_function("limb_adc_chain_1000", |b| {
        b.iter(|| {
            let (mut s, mut c) = (0u64, 0u64);
            for i in 0..1000u64 {
                let r = adc(s, i, c);
                s = black_box(r.0);
                c = r.1;
            }
            (s, c)
        })
    });
    c.bench_function("limb_sbb_chain_1000", |b| {
        b.iter(|| {
            let (mut d, mut br) = (0u64, 0u64);
            for i in 0..1000u64 {
                let r = sbb(i, d, br);
                d = black_box(r.0);
                br = r.1;
            }
            (d, br)
        })
    });
    c.bench_function("limb_mul_wide_1000", |b| {
        b.iter(|| {
            let mut acc = (0u64, 0u64);
            for i in 0..1000u64 {
                acc = mul_wide(acc.0.wrapping_add(i), 0x9E37_79B9_7F4A_7C15);
                black_box(&acc);
            }
            acc
        })
    });
}

criterion_group!(benches, bench_limb_primitives);
criterion_main!(benches);
