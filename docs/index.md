# MegaNum

Fast, exact, portable arbitrary-precision integers and floats for Python,
powered by a pure-Rust engine with an adaptive
**Sequential / Parallel / SIMD** execution layer.

- **MegaInt** — exact arbitrary-precision signed integers, validated to
  1,000,000 decimal digits.
- **MegaFloat** — arbitrary-precision floating point with user-controlled
  precision in bits and correctly-rounded core operations.
- **Dispatcher** — pure, deterministic algorithm/backend selection, tuned by
  committed benchmarks and introspectable from Python.

No GMP/MPFR linkage. No C toolchain required by users. Pure-Rust wheels for
Linux, macOS, and Windows, Python 3.10-3.14 via abi3, published on PyPI as
`meganum`.

## Design pillars

In priority order; when pillars conflict, the lower one yields.

1. **Correctness above speed.** Every optimized algorithm ships with a slow,
   obviously-correct reference twin and is differentially tested in CI.
2. **Algorithm layer orthogonal to execution layer.** Algorithms are pure
   limb-slice functions; backends supply kernels and scheduling.
3. **Adaptive dispatch.** Thresholds are data in `tunables.toml`, calibrated
   by benchmarks.
4. **Portability.** No GMP/MPFR; results are bit-identical across operating
   systems, verified by SHA-256 manifests of golden outputs.
5. **Python-first developer experience.** Python semantics, typed stubs, and
   a small, explicit public surface.

## 60-second quickstart

```bash
pip install meganum
```

```python
from meganum import MegaInt, MegaFloat, Context, Rounding

a = MegaInt("9" * 100_000)          # parse a 100,000-digit integer
b = a * a                           # dispatcher picks algorithm + backend
print(str(b)[:40], "...", len(str(b)), "digits")

with Context(precision=3_321_929, rounding=Rounding.NEAREST_EVEN):
    x = MegaFloat(2).sqrt()         # sqrt(2) at about 1M decimal digits
```

## Feature overview

| Area | What it provides |
| --- | --- |
| `MegaInt` | Exact signed integers; parse formats with `0x`/`0o`/`0b` prefixes and `_` separators; base 2-62 conversion; bytes round-trip; full operator set with Python floor semantics; number theory helpers (`gcd`, `lcm`, `modpow`, `mod_inverse`, `jacobi`, `is_probable_prime`); `isqrt`; pickle; hashing and equality compatible with `int` |
| `MegaFloat` | Precision in bits via `Context`; five rounding modes; correctly-rounded `+ - * / sqrt`; `exp`, `ln`, `log2`, `log10`, `pow` via guard digits; cached constants `pi()`, `e()`, `ln2()`; scientific/decimal/engineering formatting |
| Dispatcher | Pure function `(OpContext, Tunables) -> (algorithm, backend, grain)`; Sequential, Parallel (rayon), and SIMD (AVX2/AVX-512/NEON with scalar fallback) backends; memory-aware downgrades; `last_dispatch_info()` introspection and validated overrides |

## Where to go next

| Page | Contents |
| --- | --- |
| [User Guide](user-guide.md) | Installation, quickstart, everyday recipes, threading model |
| [MegaInt API](api-megaint.md) | Integer reference: constructors, methods, contracts |
| [MegaFloat API](api-megafloat.md) | Float, `Context`, and `Rounding` reference |
| [Algorithms](algorithms.md) | Complexity table, representation facts, NTT design, memory rules |
| [Dispatcher](dispatcher.md) | Dispatch model, tunables, overrides, bit-identity guarantee |
| [Benchmarks](benchmarks.md) | Methodology, performance budgets, regression gate |
| [Contributing](contributing.md) | Build, tests, coding standards, ADR policy |
| [ADRs](adr/README.md) | Architecture decision records |
| [Security](security.md) | Reporting path and supported versions |

## Scope of v1.0

Non-goals for the 1.0 release:

- No trigonometric functions on `MegaFloat` (out of scope for v1.0).
- No WASM target.
- No async API.
- No NumPy interop.
- No cryptographic constant-time guarantees.
