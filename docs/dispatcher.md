# Dispatcher

The dispatcher selects the algorithm, execution backend, and grain size for
every top-level operation. This page documents the decision model, the
current table, the tunables file, and the Python introspection surface.

## Decision model

The dispatcher is a **pure function**:

```
decide(OpContext, Tunables) -> (algorithm, backend, grain)
```

Same inputs always produce the same decision. The `OpContext` input
comprises:

- limb counts of the operands,
- operation class (mul, div, conversion, ...),
- data shape,
- memory budget,
- worker count: `available_parallelism` minus `worker_reservation` (the
  default reservation is one core left for the host process),
- CPU features (runtime detection result).

No hidden state influences a decision; profiling counters, caches, or
previous results are never consulted. This makes decisions reproducible,
loggable, and unit-testable in isolation.

## Initial dispatch table (v1.0 defaults)

Recalibrated at milestone M3 by benchmarks; treat the table as a snapshot of
the committed `tunables.toml`, not as code constants.

| Operation | Size condition | Algorithm | Backend |
| --- | --- | --- | --- |
| add / sub / cmp | all sizes | schoolbook carry chains | SIMD (chunked) |
| mul | n < 40 limbs | schoolbook | SIMD |
| mul | 40 <= n < 1024 limbs | Karatsuba | sequential (SIMD basecase) |
| mul | 1024 <= n < 8192 limbs | Toom-Cook-3 | sequential |
| mul | n >= 8192 limbs | NTT (Harvey-style) | parallel if workers >= 2, else sequential |
| div / divmod | small quotient | schoolbook long division | sequential |
| div / divmod | large | Burnikel-Ziegler 2n/n | parallel at large n |
| sqrt / isqrt | all sizes | Newton-Raphson | parallel inner stages |
| gcd | small operands | binary GCD | sequential |
| gcd | large operands | Lehmer | sequential |
| base conversion | all sizes | divide-and-conquer | parallel splits |

Complexities per algorithm are listed in [Algorithms](algorithms.md).

## tunables.toml template

Tunables live in
`crates/meganum-core/src/dispatch/tunables.toml`, embedded at compile time;
the storage mechanism is described in [ADR-0004](adr/0004-tunables-storage.md).

```toml
[mul]
schoolbook_max_limbs = 40
karatsuba_max_limbs = 1024
toom3_max_limbs = 8192
ntt_grain = 512

[parallel]
min_limbs_for_parallel = 4096
worker_reservation = 1
memory_lean_factor = 4

[simd]
enable_avx2 = true
enable_avx512 = "auto"
enable_neon = true
```

**Every value is a calibration target.** They exist to encode benchmark
outcomes, not opinions. Changing any value requires a calibration report
referenced from an ADR; drive-by threshold edits are rejected in review.

## Memory-aware fallback

The dispatcher projects peak memory for each candidate algorithm and
downgrades when the projection exceeds the budget:

| Operation class | Budget (fraction of combined operand bytes) |
| --- | --- |
| multiplication | 4x |
| division | 8x |
| base conversion | 6x |

Example downgrade path: Toom-3 falls back to Karatsuba when its projected
peak exceeds the multiplication budget. Downgrades are recorded in the
dispatch decision, so introspection always reports what actually ran.

## Worker awareness

The parallel backend uses the worker count computed at decision time as
`available_parallelism - worker_reservation`. The reservation keeps one core
responsive for the Python host process. Decisions that need concurrency
(such as NTT at n >= 8192 limbs) fall back to sequential execution when only
one worker is available.

## CPU feature detection

CPU features are detected **once at import** and stored in atomic flags;
kernels read the flags and never re-probe the CPU. Runtime detection covers
AVX2 and AVX-512 on x86-64 and NEON on aarch64. The scalar fallback is
always compiled, so every code path works on hardware without the optional
features. Results are bit-identical regardless of which kernel runs.

## Tracing

Building the engine with the `dispatch-trace` cargo feature logs one line
per top-level decision to stderr: operation, sizes, chosen algorithm,
backend, and grain. Tracing is a diagnostic tool; it must be disabled in
release builds used for benchmarking because output itself distorts
timings.

## Introspection and overrides

Python exposes the last top-level decision and a validated override hook:

```python
import meganum

# What did the engine choose for the previous top-level operation?
meganum.last_dispatch_info()

# Force the NTT algorithm for multiplication.
meganum.set_dispatch_overrides({"mul": "ntt"})

# Restore data-driven defaults.
meganum.reset_dispatch_overrides()
```

Overrides live in a validated enum space; an invalid algorithm or operation
name raises `meganum.DispatchError`:

```python
import meganum

meganum.set_dispatch_overrides({"mul": "not-an-algorithm"})
# meganum.DispatchError: dispatch error: unknown algorithm 'not-an-algorithm'
```

Overrides affect dispatch only; they cannot change results, because all
backends are bit-identical by contract.

## Bit identity

Sequential, Parallel, and SIMD backends produce **bit-identical** results
for identical inputs on every platform. This is guaranteed by design
(deterministic merge in the parallel backend; no approximation in SIMD
kernels) and enforced by differential tests that compare backend outputs
across the test matrix. See [Algorithms](algorithms.md#correctness-rules)
and [Benchmarks](benchmarks.md).
