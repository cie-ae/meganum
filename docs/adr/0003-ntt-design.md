# ADR 0003: NTT multiplication design

## Status

Accepted

## Date

2026-11-18

## Context

The specification mandates Harvey-style NTT multiplication for operands of
8192 limbs and above, with the design recorded in an ADR. Schoolbook,
Karatsuba, and Toom-Cook-3 cover smaller sizes, so the NTT must dominate
O(n^2)-family costs at megadigit scale while remaining deterministic and
bit-identical across the Sequential and Parallel backends.

The arithmetic constraint: for `n <= 2^19` limbs (the 10^7-digit stretch),
a convolution coefficient is bounded by
`n * (2^64 - 1)^2 < 2^145`. The transform must therefore reconstruct
products from a modulus comfortably above `2^145`, without overflow and
without ever being wrong at any size the engine accepts.

## Decision

Large multiplication uses a three-prime NTT:

- **Primes.** Three NTT-friendly primes `p_i < 2^63`, each satisfying
  `p_i = 1 (mod 2^32)`, so transform lengths up to 2^32 are supported.
  Three primes at or above `2^62` give a combined modulus of at least
  `2^186 > 2^145`, covering the coefficient bound with headroom.
- **Multiplication core.** Montgomery multiplication with `u128`
  intermediates inside every butterfly.
- **Transform shape.** Iterative Cooley-Tukey: decimation-in-frequency for
  the forward transform, decimation-in-time for the inverse, with
  precomputed twiddle tables cached per prime. The iterative form keeps
  the stack flat at megadigit sizes.
- **Padding.** Cyclic transform length `L = smallest power of two >=
  a_len + b_len`; zero padding prevents wraparound contamination.
- **Pointwise stage.** Pointwise Montgomery products in each prime field.
- **Reconstruction.** CRT in mixed-radix form using a small 256-bit
  accumulator; carry normalization then walks the coefficients with a
  256-bit accumulator.
- **Parallelization.** Butterfly levels and the pointwise/carry ranges are
  split at `ntt_grain` limbs; merges are deterministic, so output is
  bit-identical to the sequential run.
- **Placement.** Prime constants, the generator, and a verifier live in
  `algos/mul/ntt.rs` with a unit test asserting primality and root
  validity.

Threshold choice (8192 limbs) is a calibration target in `tunables.toml`
and is recalibrated by benchmarks, not fixed by this ADR.

## Consequences

- Deterministic and bit-identical results across backends by construction;
  differential tests enforce it.
- Transform length headroom extends to 2^32, far beyond the sizes the
  engine accepts in v1.0.
- Montgomery arithmetic and per-prime twiddle tables add fixed setup cost
  per operation, which is why the dispatcher only selects the NTT at
  n >= 8192 limbs.
- Recalibrating entry/exit thresholds is handled entirely through
  tunables; changing them requires a calibration report per the dispatcher
  policy.

## Evidence

- Coefficient bound: `n * (2^64 - 1)^2 < 2^145` for `n <= 2^19` limbs;
  three-prime modulus `>= 2^186` from primes `>= 2^62`.
- Spec: Harvey-style NTT mandated for `n >= 8192` limbs.
- Unit test in `algos/mul/ntt.rs` asserts primality and root validity of
  the prime constants; differential tests compare NTT output against the
  reference twin across the backend matrix.
