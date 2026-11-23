# ADR 0005: MegaInt true division semantics

## Status

Accepted

## Date

2026-11-18

## Context

CPython's `int / int` returns a float. For million-digit MegaInt operands,
a float result would be meaningless: the quotient of two huge integers
rarely fits the IEEE-754 range, and if it does, the 53-bit result discards
essentially all information. The specification's overflow policy forbids
silently returning `inf` (`float(MegaInt)` raises `OverflowError` beyond
the IEEE-754 range), so the float-quotient shortcut is also inconsistent
with the rest of the API. Users still need a predictable, non-surprising
answer for the `/` operator on integers, and Python floor-division
semantics (`//`, `%`, `divmod`) must match CPython exactly.

## Decision

- `MegaInt.__truediv__` raises `TypeError` with a message pointing to
  `MegaFloat` for arbitrary-precision quotients.
- `//`, `%`, and `divmod` follow Python floor semantics exactly: `//`
  floors, and `%` takes the sign of the divisor.
- Real-valued division belongs to `MegaFloat`, which covers arbitrary
  precision with the documented rounding modes.

## Consequences

- The API is predictable: no silent precision loss, no surprise infinities,
  and error messages guide users to the right type (`MegaFloat`) instead of
  leaving them with a diagnostic gap.
- The behavior is documented in the user guide and the `MegaInt` API
  reference, and mirrors CPython's own design for `int` (where `/`
  deliberately produces a float; here, where a float would be meaningless,
  it is a `TypeError`).
- Revisiting this decision requires a new ADR with user evidence
  demonstrating a real need for exact rational division semantics.

## Evidence

- Spec: float conversions must raise `OverflowError` beyond IEEE-754
  range; silent `inf` is forbidden.
- CPython behavior: `int / int` returns float; `MegaInt` follows the same
  philosophy where it can and refuses where a float answer would be
  meaningless.
- Tests: `MegaInt.__truediv__` raises `TypeError` with the pointer
  message; floor-division semantics are covered by differential tests
  against CPython `int`.
