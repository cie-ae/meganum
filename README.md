# MegaNum

Fast, exact, portable arbitrary-precision integers and floats for Python,
powered by a pure-Rust engine with adaptive **Sequential / Parallel / SIMD**
execution selected at runtime by an algorithm dispatcher.

- `MegaInt` - exact arbitrary-precision signed integers, validated to
  1,000,000 decimal digits.
- `MegaFloat` - arbitrary-precision floating point with user-controlled
  precision (bits) and correctly-rounded core operations.
- No GMP/MPFR linkage, no C toolchain required by users. Wheels for Linux,
  macOS and Windows, Python 3.10-3.14 (abi3).

## Quickstart

```python
from meganum import MegaInt, MegaFloat, Context, Rounding

a = MegaInt("9" * 100_000)
b = a * a                       # dispatches to the NTT backend at this size
print(str(b)[:40], "...", len(str(b)), "digits")

with Context(precision=3_321_929, rounding=Rounding.NEAREST_EVEN):
    x = MegaFloat(2).sqrt()     # sqrt(2) at ~1M decimal digits
```

## Installation

```
pip install meganum
```

Wheels cover x86_64/aarch64 across Linux (glibc/musl), macOS and Windows.
On exotic platforms install a Rust toolchain (>= 1.85) and build from the
sdist (`pip install meganum --no-binary meganum`).

## Documentation

- Docs site: https://meganum.github.io/meganum
- Algorithm overview: `docs/algorithms.md`
- Dispatcher internals: `docs/dispatcher.md`
- Benchmark methodology: `benches/METHODOLOGY.md`

## License

Dual-licensed under MIT or Apache-2.0, at your option.
