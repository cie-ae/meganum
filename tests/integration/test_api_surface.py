"""FFI surface coverage: format mini-language, float comparisons, rounding,
reflected operators, byte conversion, bit queries, pickle (spec 8.2 / FR-5).

Wherever CPython `int` has the same operation the assertions are
differential (MegaInt result == CPython int result), which is the spec's
primary correctness gate.
"""

import math
import pickle

import pytest

from meganum import ConversionError, MegaInt, ParseError


def as_native(v):
    return MegaInt(v)


# ---------------------------------------------------------------------------
# format() mini-language (differential vs CPython int formatting)
# ---------------------------------------------------------------------------

@pytest.mark.parametrize(
    "value,spec",
    [
        (255, "x"), (255, "X"), (255, "#x"), (255, "#X"), (255, "#o"), (255, "#b"),
        (0, "#x"), (0, "#o"), (0, "#b"), (255, "o"), (255, "b"), (255, "d"),
        (-255, "x"), (-255, "X"), (-255, "+x"), (-255, " d"),
        (1234567, ",d"), (1234567, "_d"), (-1234567, ",d"),
        (1234567, ">12,d"), (1234567, "<12,d"), (1234567, "^12,d"),
        (1234567, "=+12,d"), (1234567, "012,d"), (1234567, "012d"),
        (42, "08d"), (-42, "08d"), (42, "*>+10,d"), (42, "*<10,d"),
        (42, "<08d"), (42, ">08d"), (7, "=+07d"), (7, "=+7d"), (7, "*=+7d"),
        (1234567, "0=12,d"), (1234567, "0>12,d"), (7, "0=+7d"),
        (10**40, ",d"), (-(10**40), ",d"), (10**40, "x"), (10**40, "#030x"),
        (7, "=+07d"), (7, "+7d"), (7, " 7d"),
    ],
)
def test_format_matches_cpython_int(value, spec):
    assert format(as_native(value), spec) == format(value, spec)


@pytest.mark.parametrize("spec", [".2d", "5.1x", "12q", "5z", "d,d", "x,", ",x"])
def test_format_invalid_specs_raise_like_cpython(spec):
    with pytest.raises(ValueError):
        format(MegaInt(255), spec)


def test_format_precision_error_message():
    with pytest.raises(ValueError):
        format(MegaInt(1), ".2d")


# ---------------------------------------------------------------------------
# Comparisons against floats (exact, CPython parity) and specials
# ---------------------------------------------------------------------------

CP_INTS = [0, 1, 5, 6, -1, -5, -6, 2**53, 2**53 + 2, 2**100, -(2**100), 123456789]
CP_FLOATS = [
    0.0, -0.0, 0.5, -0.5, 5.0, 5.5, -5.5, 6.0, -6.0, 4.5, 1e300, -1e300,
    5e-324, float(2**53), float(2**53) + 2.0, float(2**60) + 0.5, 123456789.0,
]


@pytest.mark.parametrize("a", CP_INTS)
@pytest.mark.parametrize("b", CP_FLOATS)
def test_richcmp_float_differential(a, b):
    A = as_native(a)
    assert (A < b) == (a < b)
    assert (A <= b) == (a <= b)
    assert (A > b) == (a > b)
    assert (A >= b) == (a >= b)
    assert (A == b) == (a == b)
    assert (A != b) == (a != b)
    # reflected direction: float OP MegaInt goes through CPython float
    # semantics which are exact for int-vs-float as well
    assert (b < A) == (b < a)
    assert (b > A) == (b > a)
    assert (b == A) == (b == a)


def test_richcmp_specials_nan_inf():
    nan = float("nan")
    inf = float("inf")
    ninf = float("-inf")
    for a in [0, 5, -5, 2**100]:
        A = as_native(a)
        assert (A < nan) == (a < nan) is False
        assert (A > nan) == (a > nan) is False
        assert (A == nan) == (a == nan) is False
        assert (A != nan) == (a != nan) is True
        assert (A < inf) == (a < inf)
        assert (A <= inf) == (a <= inf)
        assert (A > ninf) == (a > ninf)
        assert (A >= ninf) == (a >= ninf)
        assert (A == inf) == (a == inf) is False
    with pytest.raises(TypeError):
        MegaInt(1) < "not a number"


def test_float_conversion_matches_cpython():
    for v in [0, 1, 42, 2**52, 2**53, 2**53 + 1, 2**53 + 2, 2**54 + 1,
              2**54 + 2, 2**54 + 3, -(2**54 + 3), 2**62, 10**15]:
        assert float(as_native(v)) == float(v)
    with pytest.raises(OverflowError):
        float(as_native(2**1024))
    with pytest.raises(OverflowError):
        float(as_native(-(2**1024)))


# ---------------------------------------------------------------------------
# hash / str / repr
# ---------------------------------------------------------------------------

def test_hash_matches_cpython_int():
    for v in [0, 1, -1, 2, 42, 2**61 - 1, 2**61, 2**61 + 1, 2**100, -(2**100),
              2**200 + 12345]:
        assert hash(as_native(v)) == hash(v)
    assert hash(MegaInt(-1)) == -2


def test_repr_short_and_long():
    assert repr(MegaInt(0)) == "MegaInt('0')"
    assert repr(MegaInt(-42)) == "MegaInt('-42')"
    long_repr = repr(MegaInt(10**120))
    assert long_repr.startswith("MegaInt('1")
    assert "..." in long_repr and "(digits=121)" in long_repr


# ---------------------------------------------------------------------------
# round / trunc / floor / ceil (differential vs CPython)
# ---------------------------------------------------------------------------

@pytest.mark.parametrize(
    "value,nd",
    [
        (1250, -2), (1350, -2), (1260, -2), (1234, -1), (1234, -3), (1234, -9),
        (-1250, -2), (-1350, -2), (-1260, -2), (0, -2), (50, -1), (150, -1),
        (999, -1), (995, -1), (1050, -2), (1150, -2), (2**80 + 55, -1),
        (1234, 0), (1234, 2),
    ],
)
def test_round_matches_cpython_int(value, nd):
    assert round(as_native(value), nd) == round(value, nd)


def test_round_no_ndigits_returns_self():
    v = as_native(-1234)
    assert round(v) == -1234


def test_trunc_floor_ceil_match_cpython():
    for v in [0, 42, -42, 2**100, -(2**100)]:
        assert math.trunc(as_native(v)) == math.trunc(v)
        assert math.floor(as_native(v)) == math.floor(v)
        assert math.ceil(as_native(v)) == math.ceil(v)


# ---------------------------------------------------------------------------
# reflected operators (int OP MegaInt -> __r*__)
# ---------------------------------------------------------------------------

def test_reflected_arithmetic_matches_cpython():
    for a, b in [(3, 7), (0, 5), (-4, 9), (2**70, 3), (10, 10**50)]:
        assert 3 * as_native(b) == 3 * b
        assert a % as_native(b) == a % b if b else True
        assert a + as_native(b) == a + b


def test_reflected_mod_and_floordiv():
    assert 10 % MegaInt(3) == 1
    assert -10 % MegaInt(3) == 2
    assert 10 % MegaInt(-3) == -2
    with pytest.raises(ZeroDivisionError):
        5 % MegaInt(0)
    with pytest.raises(ZeroDivisionError):
        5 // MegaInt(0)


def test_divmod_matches_cpython_int():
    for a, b in [(7, 3), (-7, 3), (7, -3), (-7, -3), (0, 5), (10**50, 7),
                 (2**80, 2**40), (5, 5), (4, 5)]:
        got_q, got_r = divmod(as_native(a), as_native(b))
        want_q, want_r = divmod(a, b)
        assert got_q == want_q and got_r == want_r
        got_q, got_r = divmod(as_native(a), b)
        assert got_q == want_q and got_r == want_r
    with pytest.raises(ZeroDivisionError):
        divmod(as_native(5), 0)


def test_reflected_pow_matches_cpython():
    assert 3 ** as_native(2) == 3**2
    assert 2 ** as_native(10) == 2**10
    assert pow(3, as_native(2), 5) == pow(3, 2, 5)
    with pytest.raises(TypeError):
        3 ** as_native(-1)
    with pytest.raises(ValueError):
        2 ** as_native(2**70)
    with pytest.raises(ValueError):
        2 ** as_native(100_000_000)


def test_reflected_bitwise_matches_cpython_int():
    for a, b in [(5, 3), (0, 7), (12, 10), (2**70, 2**60 + 1), (255, 256)]:
        assert a & as_native(b) == a & b
        assert a | as_native(b) == a | b
        assert a ^ as_native(b) == a ^ b
    for a, b in [(-5, 3), (-1, 7), (-8, 3)]:
        assert a & as_native(b) == a & b
        assert a | as_native(b) == a | b
        assert a ^ as_native(b) == a ^ b


def test_pos_is_copy():
    v = as_native(-42)
    assert +v == v and +v is not v


def test_compare_ne_branch():
    assert (MegaInt(1) != MegaInt(2)) is True
    assert (MegaInt(1) != MegaInt(1)) is False


# ---------------------------------------------------------------------------
# bytes conversion (differential vs int.to_bytes / int.from_bytes)
# ---------------------------------------------------------------------------

def test_from_bytes_matches_cpython_int():
    assert MegaInt.from_bytes(b"\x01\x00", "little") == 1
    assert MegaInt.from_bytes(b"\x00\x01", "big") == 1
    assert MegaInt.from_bytes(b"\xff", "little", signed=True) == -1
    assert MegaInt.from_bytes(b"\xff", "big", signed=True) == -1
    assert MegaInt.from_bytes(b"\x80", "big") == 128
    data = (2**100).to_bytes(13, "little")
    assert MegaInt.from_bytes(data, "little") == 2**100
    assert MegaInt.from_bytes(data[::-1], "big") == 2**100


def test_to_bytes_matches_cpython_int():
    for v, ln in [(0, 0), (1, 1), (300, 2), (2**32, 5), (2**100, 13), (255, 8)]:
        for bo in ("big", "little"):
            assert as_native(v).to_bytes(ln, bo) == v.to_bytes(ln, bo)
    assert as_native(-1).to_bytes(1, "big", signed=True) == (-1).to_bytes(1, "big", signed=True)
    assert as_native(-300).to_bytes(2, "little", signed=True) == (-300).to_bytes(2, "little", signed=True)
    with pytest.raises(OverflowError):
        as_native(300).to_bytes(1)
    with pytest.raises(ValueError):
        as_native(300).to_bytes(1, "middle")  # type: ignore[arg-type]


def test_from_bytes_bad_byteorder_raises():
    with pytest.raises(ValueError):
        MegaInt.from_bytes(b"\x01", "middle")  # type: ignore[arg-type]


def test_to_base_and_radix_error():
    assert MegaInt(255).to_base(16) == "ff"
    assert MegaInt(255).to_base(36) == "73"
    assert MegaInt(0).to_base(2) == "0"
    with pytest.raises(ValueError):
        MegaInt(255).to_base(1)
    with pytest.raises(ValueError):
        MegaInt(255).to_base(63)


# ---------------------------------------------------------------------------
# bit queries (FR-1.3)
# ---------------------------------------------------------------------------

def test_bit_queries():
    assert MegaInt(0).bit_length() == 0
    assert MegaInt(10).bit_length() == 10 .bit_length()
    assert MegaInt(255).bit_length() == 8
    assert MegaInt(-255).bit_length() == 8
    assert MegaInt(255).bit_count() == 8
    assert MegaInt(0).bit_count() == 0
    assert MegaInt(10).trailing_zeros() == 1
    assert MegaInt(7).trailing_zeros() == 0
    assert MegaInt(0).trailing_zeros() is None
    # leading_zeros: 64 * limb_count - bit_length (zero-padded to limbs)
    assert MegaInt(255).leading_zeros() == 64 - 8
    assert MegaInt(2**64).leading_zeros() == 128 - 65
    assert MegaInt(0).leading_zeros() == 0


# ---------------------------------------------------------------------------
# construction / pickle / misc
# ---------------------------------------------------------------------------

def test_default_constructor_is_zero():
    assert MegaInt() == 0


def test_pickle_roundtrip_and_invalid_payload():
    for v in [0, 1, -1, 42, -(10**40), 2**100]:
        assert pickle.loads(pickle.dumps(as_native(v))) == v
    native = __import__("meganum._native", fromlist=["_native"])
    with pytest.raises(TypeError):
        native._unpickle_megaint(b"not-a-payload")
    with pytest.raises(TypeError):
        native._unpickle_megaint(b"m1")


def test_power_guard_paths():
    # 2-arg ** negative exponent: TypeError pointing at MegaFloat
    with pytest.raises(TypeError):
        as_native(2) ** -1
    # result would exceed the ~8 MB guard: PrecisionError (a ValueError)
    with pytest.raises(ValueError):
        as_native(2) ** 100_000_000
    # modular pow with negative exponent computes the exact inverse
    inv = pow(as_native(3), -1, 7)
    assert inv == pow(3, -1, 7)
