# MegaFloat, Context, and Rounding API Reference

!!! note "Handwritten summary for v1.0"

    These pages are handwritten summaries, not generated docs; mkdocstrings
    is intentionally not enabled for v1.0. Complete signatures and the
    authoritative descriptions live in the docstrings and in the stub file
    `python/meganum/__init__.pyi` (pyright strict-clean). Where a docstring
    states a rounding guarantee for an operation, the docstring decides.

## MegaFloat construction

| Form | Description |
| --- | --- |
| `MegaFloat("3.14")` | From a decimal string |
| `MegaFloat(2)` | From an `int` |
| `MegaFloat(some_megaint)` | From a `MegaInt` |
| `MegaFloat(other_megafloat)` | From another `MegaFloat` |

```python
from meganum import MegaFloat

half = MegaFloat("0.5")
big = MegaFloat(MegaInt(10) ** 500)     # exact at construction
```

## Context

A `Context` fixes the floating-point environment for a block:

| Field | Description |
| --- | --- |
| `precision` | Working precision in **bits** for the significand |
| `rounding` | One of the `Rounding` modes below |

- Without a `Context` block, a thread-local default applies: **256 bits,
  `NEAREST_EVEN`**.
- Contexts nest; the **innermost active context wins** for the duration of
  its block.
- Contexts are immutable and entering one is O(1).

```python
from meganum import MegaFloat, Context, Rounding

with Context(precision=3_321_929, rounding=Rounding.NEAREST_EVEN):
    root = MegaFloat(2).sqrt()          # sqrt(2) at ~1M decimal digits
```

One million decimal digits of precision correspond to 3,321,929 bits.

## Rounding modes

| Mode | Behavior |
| --- | --- |
| `Rounding.NEAREST_EVEN` | To nearest; ties to the even significand (round-half-even) |
| `Rounding.NEAREST_AWAY` | To nearest; ties away from zero |
| `Rounding.FLOOR` | Toward negative infinity |
| `Rounding.CEILING` | Toward positive infinity |
| `Rounding.UP` | Away from zero |
| `Rounding.DOWN` | Toward zero |

Overflow and underflow follow the active rounding mode: the rounding
direction decides whether a value saturates to the mode's infinity or
rounds to the largest representable magnitude.

## Operations

| Group | Members |
| --- | --- |
| Arithmetic | `+ - * /` and unary negation, `abs` |
| Power | `**`, `pow` |
| Roots | `sqrt` |
| Exponentials and logs | `exp`, `ln`, `log2`, `log10` |

```python
from meganum import MegaFloat, Context, Rounding

with Context(precision=167, rounding=Rounding.NEAREST_EVEN):
    x = MegaFloat("2").sqrt()
    y = x * x                           # close to 2 at 167-bit precision
    z = MegaFloat(2) ** MegaFloat("0.5")
```

## Constants

| Constant | Description |
| --- | --- |
| `MegaFloat.pi()` | Correctly rounded on demand at the active precision |
| `MegaFloat.e()` | Correctly rounded on demand at the active precision |
| `MegaFloat.ln2()` | Correctly rounded on demand at the active precision |

Constants are computed when first requested at a given environment and
cached per `(precision, rounding)` pair; repeated requests at the same
environment reuse the cached value.

## Formatting

MegaFloat supports directed formatting:

```python
from meganum import MegaFloat

y = MegaFloat("3.14")
format(y, precision=50, notation="sci")
```

| Parameter | Values |
| --- | --- |
| `precision` | Number of significant digits to render |
| `notation` | `auto` (automatic choice), `dec` (decimal), `sci` (scientific), `eng` (engineering) |

`int(f)` truncates toward zero.

## Exceptions and error behavior

- Requests beyond the supported precision envelope raise `PrecisionError`
  (a `ValueError` subclass).
- Division by zero raises `DivisionByZeroError` (a `ZeroDivisionError`
  subclass).
- Overflow and underflow follow the **active rounding mode** rather than a
  fixed direction.
- All exceptions derive from `MeganumError`; see the
  [User Guide exception table](user-guide.md#exceptions).

## Correctly-rounded policy

MegaNum applies a strict claim policy:

- Core operations (`+ - * / sqrt`) are **correctly rounded**: rounding
  decisions are made from exact integer evidence, with a proven error bound
  of at most 1/2 ulp.
- `exp`, `ln`, `log2`, `log10`, and `pow` are computed with guard digits and
  Ziv-style recomputation; each operation documents its guarantee in its
  docstring. Operations without a proven bound are documented as *rounded
  per context*.
- Trigonometric functions are out of scope for v1.0.
