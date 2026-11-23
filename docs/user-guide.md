# User Guide

## Installation

MegaNum ships prebuilt wheels; installing requires no Rust toolchain and no
C compiler:

```bash
pip install meganum
```

### Wheel matrix

The wheels are abi3 wheels tagged `cp310`; one wheel covers Python
3.10 through 3.14 on each platform.

| Platform | Tags | Status |
| --- | --- | --- |
| Linux x86_64 (glibc) | `manylinux2014`, `manylinux_2_28` | Supported |
| Linux aarch64 (glibc) | `manylinux2014`, `manylinux_2_28` | Supported |
| Linux x86_64 (musl) | `musllinux_1_2` | Supported |
| Linux aarch64 (musl) | `musllinux_1_2` | Supported |
| macOS x86_64 | `macosx` | Supported |
| macOS arm64 (Apple Silicon) | `macosx` | Supported |
| Windows x86_64 | `win_amd64` | Supported |
| Windows aarch64 | `win_arm64` | Best-effort; build from the sdist if no wheel matches |

### Building from the sdist

If no wheel matches your platform, MegaNum builds from source. This requires
a Rust toolchain of **1.85 or newer**; the build itself takes a few minutes
on a modern laptop.

```bash
pip install meganum --no-binary meganum
```

## Quickstart

```python
from meganum import MegaInt, MegaFloat, Context, Rounding

a = MegaInt("9" * 100_000)
b = a * a                       # dispatches to the NTT backend at this size
print(str(b)[:40], "...", len(str(b)), "digits")

with Context(precision=3_321_929, rounding=Rounding.NEAREST_EVEN):
    x = MegaFloat(2).sqrt()     # sqrt(2) at ~1M decimal digits
```

## MegaInt

### Construction and parsing

```python
from meganum import MegaInt

MegaInt(123)                    # from int
MegaInt("9" * 1_000_000)        # from decimal string
MegaInt("0x1F")                 # optional 0x / 0o / 0b prefixes
MegaInt("1_234_567")            # underscore separators
MegaInt.from_hex("deadbeef")
MegaInt.from_bin("1010101")
MegaInt.from_oct("777")
MegaInt.from_base("z9", radix=62)   # radix 2..62
MegaInt.from_bytes(b"\x01\x00", byteorder="little", signed=False)
```

String parsing accepts decimal input with optional `0x`/`0o`/`0b` prefixes
and `_` separators between digits. `from_base` covers radix 2 through 62,
using digits `0-9`, then `a-z`, then `A-Z`.

### Arithmetic

All operators follow Python integer semantics:

```python
a = MegaInt(17)
b = MegaInt(5)

a + b                           # MegaInt(22)
a - b
a * b
a // b                          # floor division, MegaInt(3)
a % b                           # result has the sign of the divisor
divmod(a, b)
a ** b                          # exponent must be non-negative
a << b
a >> b
a & b
a | b
a ^ b
~a                              # bitwise negation
-a                              # negation
abs(a)
a == 17                         # comparisons work against int
-a < 0
```

Semantics to rely on:

- `//` and `%` implement floor division and mod-with-sign-of-divisor,
  exactly like CPython `int`.
- `**` requires a non-negative exponent; a negative exponent raises
  `TypeError` pointing to `MegaFloat`, like CPython `int`.
- MegaInt does not implement true division; `a / b` raises `TypeError` with
  a message pointing to `MegaFloat` (see
  [ADR-0005](adr/0005-megaint-truediv.md)).
- `MegaInt op int` returns a `MegaInt`; the `int` operand is converted
  exactly.

### Number theory helpers

```python
a.gcd(b)                        # greatest common divisor
a.lcm(b)
a.modpow(exp, mod)              # modular exponentiation (Montgomery)
a.mod_inverse(mod)              # modular inverse, raises on non-invertible
a.jacobi(n)                     # Jacobi symbol
a.is_probable_prime(rounds)     # Miller-Rabin primality screening
```

### Bitwise queries and roots

```python
a.bit_length()
a.bit_count()                   # number of set bits
a.trailing_zeros()
a.leading_zeros()
a.isqrt()                       # integer square root
```

### Conversion and output

```python
str(a)                          # exact decimal string
a.to_hex()                      # hexadecimal string
a.to_bin()                      # binary string
a.to_base(62)                   # radix 2..62
bytes_ = a.to_bytes(length=None, byteorder="big", signed=False)
a2 = MegaInt.from_bytes(bytes_, byteorder="big", signed=False)   # round-trip
int(a)                          # exact Python int
float(a)                        # raises OverflowError beyond IEEE-754 range
bool(a)                         # False only for zero
round(a, ndigits)
```

`int(a)` is exact for any size; it converts through the limb representation
and is never affected by `sys.set_int_max_str_digits`. See
[Interoperability notes](#interoperability-notes) for the one place the
CPython digit limit can still surface.

### Formatting, hashing, serialization

- `repr(a)` truncates the middle with `...` for values above roughly 80
  digits and states the digit count.
- The format mini-language supports a subset: fill, align, sign, and comma
  grouping, e.g. `format(a, ",")`.
- `hash(MegaInt(5)) == hash(5)` and `MegaInt(5) == 5`; hash and equality are
  compatible with `int`.
- Comparisons against `float` are exact rational comparisons; no hidden
  rounding.
- Values are immutable; operations return new values.
- Pickle round-trips under protocol 5 and earlier. `copy` and `deepcopy`
  also work.

## MegaFloat and Context

### Construction

```python
from meganum import MegaFloat, Context, Rounding

MegaFloat("3.14")               # from decimal string
MegaFloat(2)                    # from int
MegaFloat(MegaInt(10) ** 500)   # from MegaInt
MegaFloat(MegaFloat("1.5"))     # from MegaFloat
```

### Precision and rounding

`MegaFloat` uses a decimal context model: precision is expressed in **bits**,
set per block with `Context`:

```python
with Context(precision=3_321_929, rounding=Rounding.NEAREST_EVEN):
    x = MegaFloat(2).sqrt()     # sqrt(2) at ~1M decimal digits
```

One million decimal digits of precision correspond to 3,321,929 bits.

Without a `Context` block, a thread-local default applies: **256 bits,
`NEAREST_EVEN`**.

### Rounding modes

| Mode | Behavior |
| --- | --- |
| `Rounding.NEAREST_EVEN` | Round to nearest; ties go to the even significand (round-half-even) |
| `Rounding.NEAREST_AWAY` | Round to nearest; ties go away from zero |
| `Rounding.FLOOR` | Round toward negative infinity |
| `Rounding.CEILING` | Round toward positive infinity |
| `Rounding.UP` | Round away from zero |
| `Rounding.DOWN` | Round toward zero |

Overflow and underflow follow the active rounding mode.

### Context nesting

Contexts nest; the innermost active context wins for the duration of its
block. Entering a context is O(1), and contexts are immutable once created.

```python
with Context(precision=167, rounding=Rounding.NEAREST_EVEN):
    a = MegaFloat(2).sqrt()             # computed at 167 bits
    with Context(precision=333, rounding=Rounding.NEAREST_EVEN):
        b = MegaFloat(2).sqrt()         # computed at 333 bits
    c = MegaFloat(2).sqrt()             # back at 167 bits
```

### Constants and operations

- Constants: `MegaFloat.pi()`, `MegaFloat.e()`, `MegaFloat.ln2()`, each
  correctly rounded on demand at the active precision and cached per
  `(precision, rounding)` pair.
- Operations: `+ - * / neg abs ** sqrt exp ln log2 log10 pow`.
- `int(f)` truncates toward zero.
- Formatting supports `format(y, precision=50, notation="sci")` with
  `notation` one of `auto`, `dec`, `sci`, `eng`.

Trigonometric functions are out of scope for v1.0.

## Threading

- All public value types are immutable and safe to share across threads
  (`Send + Sync` on the Rust side).
- The GIL is released for native operations expected to exceed 1 ms, so
  heavy computations run concurrently.
- Expected scaling follows core count and is workload-dependent; the
  documented contract is **P14: 4-thread GIL-free scaling of at least 2.5x**
  on the reference benchmark hardware (see [Benchmarks](benchmarks.md)).
  Real speedup on other machines depends on available cores and system load.
- The dispatcher reads the worker count once per decision as
  `available_parallelism - worker_reservation` (one core reserved for the
  host); see [Dispatcher](dispatcher.md).

## Exceptions

All MegaNum exceptions derive from `MeganumError`.

| Exception | Inherits from | Raised when |
| --- | --- | --- |
| `MeganumError` | `Exception` | Base class of the exception hierarchy |
| `ParseError` | `ValueError` | A string or byte payload cannot be parsed |
| `DivisionByZeroError` | `ZeroDivisionError` | A division or modulo has a zero divisor |
| `ConversionError` | `ValueError`, `OverflowError` | A conversion is lossy or out of range |
| `PrecisionError` | `ValueError` | A precision request exceeds the supported envelope |
| `DispatchError` | `RuntimeError` | An invalid dispatch override is supplied |

Catch the hierarchy root when the specific cause does not matter:

```python
from meganum import MeganumError, MegaInt

try:
    MegaInt("not a number")
except MeganumError:
    ...
```

Because the subclasses derive from standard builtins, common patterns keep
working: `except ValueError` catches `ParseError`, and
`except ZeroDivisionError` catches `DivisionByZeroError`.

## Interoperability notes

- `int(meganum_value)` is **exact** and converts through the limb path; it
  is never limited by `sys.set_int_max_str_digits`.
- However, calling `str()` on the resulting **Python** `int` *is* subject to
  CPython's `sys.set_int_max_str_digits` limit, because at that point it is
  a plain CPython integer. MegaNum's own `str(MegaInt)` is unaffected.
- `float(MegaInt)` raises `OverflowError` beyond the IEEE-754 double range;
  it never silently returns `inf`.
- `MegaInt op int` returns `MegaInt`, so mixing the two types keeps full
  precision.
