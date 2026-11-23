# MegaInt API Reference

!!! note "Handwritten summary for v1.0"

    These pages are handwritten summaries, not generated docs; mkdocstrings
    is intentionally not enabled for v1.0. Complete signatures and the
    authoritative descriptions live in the docstrings and in the stub file
    `python/meganum/__init__.pyi` (pyright strict-clean). This page orients
    you; the docstrings decide.

## Constructors

| Form | Description |
| --- | --- |
| `MegaInt(123)` | From a Python `int` |
| `MegaInt("9" * 1_000_000)` | From a decimal string; `_` separators allowed |
| `MegaInt("0x1F")` / `MegaInt("0o17")` / `MegaInt("0b101")` | Strings with optional `0x` / `0o` / `0b` prefixes |
| `MegaInt.from_hex(s)` | From a hexadecimal string |
| `MegaInt.from_bin(s)` | From a binary string |
| `MegaInt.from_oct(s)` | From an octal string |
| `MegaInt.from_base(s, radix=2..62)` | From a string in radix 2 through 62 |
| `MegaInt.from_bytes(b, byteorder="little"/"big", signed=False)` | From bytes |

```python
from meganum import MegaInt

x = MegaInt("0xdead_beef")            # prefix plus underscore separators
y = MegaInt.from_base("Z9", radix=62)
z = MegaInt.from_bytes(b"\x02\x01", byteorder="little")
```

Invalid payloads raise `ParseError`.

## Arithmetic operators

`+ - * // % ** << >> & | ^ ~`, unary negation, `abs`, `divmod`, and
comparisons against `int` (and `MegaInt`). All results are new, immutable
`MegaInt` values.

| Operator | Semantics |
| --- | --- |
| `a + b`, `a - b`, `a * b` | Exact integer arithmetic |
| `a // b`, `a % b`, `divmod(a, b)` | Floor division; `a % b` has the sign of the divisor (Python semantics) |
| `a ** b` | Non-negative exponents only; negative raises `TypeError` pointing to `MegaFloat` |
| `a << b`, `a >> b` | Shifts |
| `a & b`, `a \| b`, `a ^ b`, `~a` | Bitwise two's-complement semantics |
| `-a`, `abs(a)` | Sign manipulation |
| `a / b` | Not supported; raises `TypeError` pointing to `MegaFloat` (see [ADR-0005](adr/0005-megaint-truediv.md)) |
| `== != < <= > >=` | Against `int` and `MegaInt`; comparisons against `float` are exact rational comparisons |

```python
from meganum import MegaInt

a = MegaInt(17)
b = MegaInt(5)
a % b                    # MegaInt(2): sign follows the divisor
a // b                   # MegaInt(3): floor
a ** MegaInt(64)
```

## Number theory

| Method | Description |
| --- | --- |
| `a.gcd(b)` | Greatest common divisor (binary GCD for small operands, Lehmer for large) |
| `a.lcm(b)` | Least common multiple |
| `a.modpow(exp, mod)` | Modular exponentiation via Montgomery multiplication |
| `a.mod_inverse(mod)` | Multiplicative inverse modulo `mod` |
| `a.jacobi(n)` | Jacobi symbol |
| `a.is_probable_prime(rounds)` | Miller-Rabin screening with the requested round count |

```python
a = MegaInt(10) ** 100 + 267
a.modpow(MegaInt(3), MegaInt(1_000_007))
a.gcd(MegaInt(60))
```

## Bitwise queries and roots

| Method | Description |
| --- | --- |
| `a.bit_length()` | Number of bits required to represent the value |
| `a.bit_count()` | Number of set bits |
| `a.trailing_zeros()` | Count of trailing zero bits |
| `a.leading_zeros()` | Count of leading zero bits in the top limb |
| `a.isqrt()` | Integer square root (floor) |

```python
x = MegaInt(1) << 1000
x.bit_length()           # 1001
x.bit_count()            # 1
x.isqrt()                # MegaInt(1) << 500
```

## Conversions

| Method / function | Description |
| --- | --- |
| `str(a)` | Exact decimal string, any size |
| `a.to_hex()` | Hexadecimal string |
| `a.to_bin()` | Binary string |
| `a.to_base(radix)` | String in radix 2 through 62 |
| `a.to_bytes(length=None, byteorder="big", signed=False)` | Fixed-length (or minimal) byte encoding |
| `int(a)` | Exact Python `int` via the limb path; never limited by `sys.set_int_max_str_digits` |
| `float(a)` | IEEE-754 double; raises `OverflowError` beyond range, never returns `inf` |
| `bool(a)` | False only for zero |
| `round(a, ndigits)` | Rounded per Python rounding semantics |

```python
a = MegaInt(1_000_000)
a.to_hex()                       # 'f4240'
a.to_base(36)
a.to_bytes(byteorder="big")
int(a) == 1_000_000
```

## Serialization

| Mechanism | Description |
| --- | --- |
| `pickle` | Round-trips under protocol 5 and earlier |
| `copy.copy`, `copy.deepcopy` | Supported; values are immutable, so copies share limbs safely |
| `repr(a)` | Truncates the middle with `...` for values above roughly 80 digits and reports the digit count |
| format mini-language | Subset: fill, align, sign, comma grouping (e.g. `format(a, ",")`) |

```python
import pickle
from meganum import MegaInt

x = MegaInt(10) ** 500
y = pickle.loads(pickle.dumps(x))    # == x
```

## Semantic contracts

- **Immutability.** Values are immutable; every operation returns a new
  value. Instances are safe to share across threads.
- **Floor semantics.** `//` and `%` match CPython: `//` floors, `%` takes
  the sign of the divisor.
- **Hash and equality vs int.** `hash(MegaInt(5)) == hash(5)` and
  `MegaInt(5) == 5`; MegaInt is a drop-in value inside dicts and sets
  alongside plain ints.
- **Repr truncation.** `repr` never dumps megadigit strings; it truncates
  the middle and states the total digit count.
- **Pickle.** Protocol 5 and earlier round-trip exactly.
- **Exceptions.** Failures raise the documented hierarchy rooted at
  `MeganumError` (`ParseError`, `DivisionByZeroError`, `ConversionError`;
  see [User Guide](user-guide.md#exceptions)).

For exception classes, the `about()` payload, and dispatch introspection,
see the [User Guide](user-guide.md) and [Dispatcher](dispatcher.md).
