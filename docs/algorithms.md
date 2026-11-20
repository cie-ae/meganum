# Algorithms

This page summarizes the algorithm inventory, the underlying representation,
and the correctness and memory rules that govern engine code. Thresholds and
dispatch decisions are covered in [Dispatcher](dispatcher.md).

## Complexity overview

`M(n)` denotes the cost of multiplying two n-limb values; it is the yardstick
for subquadratic algorithms.

| Operation | Algorithm (initial dispatch) | Complexity |
| --- | --- | --- |
| add / sub / cmp | schoolbook carry chains | O(n) |
| mul, n < 40 limbs | schoolbook | O(n^2) |
| mul, 40 <= n < 1024 limbs | Karatsuba | O(n^1.585) |
| mul, 1024 <= n < 8192 limbs | Toom-Cook-3 | O(n^1.465) |
| mul, n >= 8192 limbs | NTT (Harvey-style) | O(n log n) |
| div / divmod, small quotient | schoolbook long division | quadratic |
| div / divmod, large | Burnikel-Ziegler 2n/n | O(M(n)) |
| sqrt / isqrt | Newton-Raphson (parallel inner stages) | O(M(n)) |
| gcd, small | binary GCD | O(n^2) worst case, subquadratic in practice |
| gcd, large | Lehmer | O(n^2) worst case, subquadratic in practice |
| base conversion | divide-and-conquer | O(M(n) log n) |
| modpow | Montgomery | O(M(n) log e) |

The default table is recalibrated at milestone M3 from committed benchmark
reports; the thresholds themselves live in `tunables.toml`, not in code.

## Representation quick reference

| Fact | Value |
| --- | --- |
| Limb type | `u64`, base 2^64, little-endian ordering |
| Canonical zero | empty limb vector |
| 1M decimal digits | ~52,000 limbs, ~3,321,929 bits |
| Float precision | in bits, set via `Context` |
| Mul thresholds (initial) | schoolbook < 40 limbs; Karatsuba < 1024; Toom-3 < 8192; NTT >= 8192 |

The limb layout is locked: changing base or ordering would invalidate every
backend, the NTT bound analysis below, and the golden vector manifests.

## Correctness rules

1. **Reference twins are the primary correctness gate.** Every optimized
   algorithm ships with a slow, obviously-correct reference implementation
   under `algos/_ref`. CI differentially tests optimized code against these
   twins, in addition to property-based tests (`proptest`), differential
   tests against CPython `int`/`decimal`, golden vectors with a SHA-256
   manifest, `cargo-fuzz` targets, and nightly ASAN/TSAN runs.
2. **Correctly-rounded claim policy.** An operation may be documented as
   *correctly rounded* only with a proven error bound of at most 1/2 ulp,
   verified at runtime by a Ziv-style rounding test. Operations without such
   a proof are documented as *rounded per context*; the docstring states
   which policy applies.
3. **Bit identity across backends.** Sequential, Parallel, and SIMD
   execution must produce identical limbs for identical inputs. Differential
   tests compare backend outputs on every run; see
   [Dispatcher](dispatcher.md#bit-identity).

## NTT design summary

The large-operand multiplier is a Harvey-style number-theoretic transform.

- Three NTT-friendly primes, each `p = 1 (mod 2^32)` and below `2^63`, so
  transform lengths up to 2^32 are supported.
- Montgomery multiplication with `u128` intermediates inside the butterflies.
- Convolution coefficients are bounded by `n * (2^64 - 1)^2 < 2^145` for
  `n <= 2^19` limbs (the 10^7-digit stretch). Three primes at or above
  `2^62` give a combined modulus of at least `2^186`, comfortably above the
  bound.
- CRT reconstruction in mixed-radix form accumulates with a small 256-bit
  accumulator before carry normalization.

The full design, including padding, parallelization, and the primality/
root-validity tests for the prime constants, is recorded in
[ADR-0003](adr/0003-ntt-design.md).

## Memory discipline

- **Pooled scratch.** Algorithms allocate working buffers from a shared
  scratch pool; repeated operations reuse buffers instead of churning the
  allocator.
- **Memory budgets.** The dispatcher downgrades an algorithm when the
  projected peak memory would exceed the budget: multiplication is allowed
  at most 4x the combined operand bytes, division at most 8x, and base
  conversion at most 6x. Example downgrade path: Toom-3 falls back to
  Karatsuba. See [Dispatcher](dispatcher.md#memory-aware-fallback).
- **Depth caps.** Recursion is bounded by construction: Toom-Cook recursion
  depth is at most 3; the NTT is iterative and allocates no stack frames
  per butterfly; divide-and-conquer base conversion bounds its split depth
  explicitly.
- **Stack safety.** Depth caps and the iterative NTT keep recursion off the
  deep-recursion path; documented caps are enforced and tested so that
  million-digit workloads cannot exhaust the stack.
- **Allocation failure is graceful.** If an allocation fails, the engine
  returns `MegaNumError::OutOfMemory` ("out of memory"), which the Python
  layer surfaces through the documented exception hierarchy. The engine
  never aborts on allocation failure.
