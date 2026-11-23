"""MegaInt operator semantics vs CPython int (spec 8.2 / 8.6)."""

import pytest

from tests.conftest import as_native, random_int


def test_small_fixed_cases():
    cases = [
        (0, 0), (1, -1), (-1, 1), (123, 456), (-123, 456),
        (2**31, -2**31), (2**63, 2**63), (10**40, 3),
    ]
    for a, b in cases:
        A, B = as_native(a), as_native(b)
        assert int(A + B) == a + b
        assert int(A - B) == a - b
        assert int(A * B) == a * b
        assert int(-A) == -a
        assert int(abs(A)) == abs(a)
        assert int(~A) == ~a
        assert (A == B) == (a == b)
        assert (A < B) == (a < b)
        assert (A <= B) == (a <= b)
        assert (A > B) == (a > b)
        assert (A >= B) == (a >= b)


def test_random_arithmetic_parity(rng):
    for trial in range(120):
        a = random_int(rng)
        b = random_int(rng)
        A, B = as_native(a), as_native(b)
        assert int(A + B) == a + b, (a, b, "add")
        assert int(a + B) == a + b, (a, b, "radd")
        assert int(A - B) == a - b, (a, b, "sub")
        assert int(a - B) == a - b, (a, b, "rsub")
        assert int(A * B) == a * b, (a, b, "mul")
        if b != 0:
            assert int(A // B) == a // b, (a, b, "floordiv")
            assert int(a // B) == a // b, (a, b, "rfloordiv")
            assert int(A % B) == a % b, (a, b, "mod")
            assert (int(A // B), int(A % B)) == divmod(a, b), (a, b, "divmod")


def test_bitwise_and_shifts(rng):
    for trial in range(80):
        a = random_int(rng)
        b = random_int(rng, choices=(16, 64))
        s = rng.randrange(0, 3000)
        A, B = as_native(a), as_native(b)
        assert int(A & B) == a & b
        assert int(A | B) == a | b
        assert int(A ^ B) == a ^ b
        assert int(A << s) == a << s
        assert int(A >> s) == a >> s
        with pytest.raises(TypeError):
            A << -1


def test_pow_semantics(rng):
    for trial in range(40):
        base = random_int(rng, choices=(4, 16, 64))
        e = rng.randrange(0, 200)
        A = as_native(base)
        assert int(A**e) == base**e
    # Negative exponent without modulus is refused (documented ADR-0005
    # companion: CPython would escape to float; MegaNum never rounds).
    with pytest.raises(TypeError):
        as_native(2) ** -1
    # Exponent-size guard (PrecisionError).
    from meganum import PrecisionError

    with pytest.raises(PrecisionError):
        as_native(2**64) ** (2**26)


def test_true_division_refused():
    with pytest.raises(TypeError):
        as_native(1) / as_native(2)


def test_conversions():
    A = as_native(2**70 + 7)
    assert int(A) == 2**70 + 7
    assert bool(as_native(0)) is False
    assert bool(as_native(1)) is True
    assert float(as_native(2**62)) == float(2**62)
    with pytest.raises(OverflowError):
        float(as_native(10**400))
    # __index__ protocol: list indexing must accept MegaInt.
    assert [10, 20, 30][as_native(1)] == 20


def test_rich_comparison_with_float(rng):
    # Exact comparison above 2**53 (FR-5.2): results must match CPython's
    # exact int-vs-float semantics, not truncated mantissa arithmetic.
    big = 2**70 + 12345
    assert (as_native(big) < 2.0**70.5) == (big < 2.0**70.5)
    # float(big) rounds down to 2**70 (12345 < half-ulp), so equality is
    # False on both sides.
    assert (as_native(big) == float(big)) == (big == float(big))
    assert (as_native(big) == 2.0**70) == (big == 2.0**70)
    assert (as_native(1) == float("nan")) is False
    assert (as_native(1) != float("nan")) is True
    assert (as_native(10**30) < float("inf")) is True
    assert (as_native(10**30) > float("-inf")) is True


def test_hash_parity(rng):
    for trial in range(40):
        v = random_int(rng, choices=(32, 128, 400))
        assert hash(as_native(v)) == hash(v)


def test_bool_int_float_dunders():
    A = as_native(42)
    assert A.__bool__() is True
    assert int(A) == 42
    assert str(A) == "42"
    assert repr(A) == "MegaInt('42')"
