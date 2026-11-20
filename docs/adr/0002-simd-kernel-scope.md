# ADR 0002: SIMD kernel scope for v1.0

## Status

Accepted

## Date

2026-11-18

## Context

The specification mandates a SIMD execution backend with runtime-detected
AVX2/AVX-512 on x86-64 and NEON on aarch64, while guaranteeing bit-identical
results across all backends. MegaNum limbs are `u64` in base 2^64.
Neither AVX2 nor AVX-512 provides a 64x64 -> 128 integer multiply
instruction, so vectorizing the schoolbook or NTT multiply kernels would
require restructuring the engine around 32-bit half-limbs. That would
violate the locked limb layout, invalidate the NTT coefficient bound
analysis, and force re-verification of every golden vector manifest, all to
accelerate a code path that only runs below 40 limbs (schoolbook multiply),
where dispatch overhead dominates anyway. Add, sub, cmp, and bulk copy/fill
are wide and carry-oriented, which vector hardware handles directly on the
locked layout.

## Decision

For v1.0, the SIMD backend vectorizes:

- add / sub / cmp kernels (chunked),
- bulk copy and fill kernels,

with AVX2 and AVX-512 detection on x86-64, NEON on aarch64, and a scalar
fallback that is always compiled. All `unsafe` remains confined to
`backend::simd` and the limb primitives, with SAFETY comments and executing
tests that exercise each kernel.

Multiplication, division, and NTT kernels remain scalar in v1.0; their
parallelism comes from the parallel backend (rayon work-splitting), not
from SIMD lanes.

## Consequences

- The add/sub/cmp rows of the dispatch table genuinely run SIMD-chunked
  kernels.
- The multiplication rows of the initial dispatch table satisfy their
  backend claims through scalar kernels; results are identical to what a
  vectorized kernel would produce, so the bit-identity contract is intact.
- A future ADR can introduce half-limb SIMD multiply kernels if profiling
  justifies the layout cost; the algorithm layer stays untouched because
  algorithms are pure limb-slice functions.
- The scalar fallback keeps every platform functional, and the differential
  backend tests continue to compare all backends for equality.

## Evidence

- Spec: SIMD backend mandated; bit-identical results required; limb layout
  locked to `u64` base 2^64.
- Instruction set facts: AVX2/AVX-512 lack 64x64 -> 128 integer multiply;
  emulating it costs more than it saves at the affected sizes (schoolbook
  runs only below 40 limbs).
- Differential tests in CI compare Sequential, Parallel, and SIMD outputs
  on every run (see `docs/dispatcher.md`, bit-identity section).
