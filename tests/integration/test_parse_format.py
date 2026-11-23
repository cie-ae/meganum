"""Parsing, radix conversion, formatting and byte serialisation (FR-1.3)."""

import pytest

import meganum
from meganum import MegaInt


def test_parse_decimal_and_underscores():
    assert int(MegaInt.parse("0")) == 0
    assert int(MegaInt.parse("1_000_007")) == 1000007
    assert int(MegaInt.parse("-42")) == -42
    assert int(MegaInt.parse("+42")) == 42
    with pytest.raises(ValueError):
        MegaInt.parse("12x34")
    with pytest.raises(ValueError):
        MegaInt.parse("1__2")
    with pytest.raises(ValueError):
        MegaInt.parse("_12")


def test_parse_prefixes():
    assert int(MegaInt.parse("0xdeadbeef")) == 0xDEADBEEF
    assert int(MegaInt.parse("0o777")) == 0o777
    assert int(MegaInt.parse("0b1011")) == 0b1011
    assert int(MegaInt.parse("0x_dead_beef")) == 0xDEADBEEF


def test_from_base_family():
    assert int(MegaInt.from_hex("dead_beef")) == 0xDEADBEEF
    assert int(MegaInt.from_bin("1011")) == 0b1011
    assert int(MegaInt.from_oct("777")) == 0o777
    assert int(MegaInt.from_base("zZ", 36)) == int("zz", 36)
    assert int(MegaInt.from_base("-ff", 16)) == -255
    # 'z' == digit 35, out of range for radix 35 (digits 0..34).
    with pytest.raises(ValueError):
        MegaInt.from_base("z", 35)
    with pytest.raises(ValueError):
        MegaInt.from_base("1", 1)
    with pytest.raises(ValueError):
        MegaInt.from_base("1", 63)


def test_roundtrip_radix_2_to_62():
    v = 123456789012345678901234567890
    V = MegaInt(v)
    for radix in range(2, 63):
        s = V.to_base(radix)
        assert int(MegaInt.from_base(s, radix)) == v, radix


def test_to_hex_to_bin():
    assert MegaInt(255).to_hex() == "ff"
    assert MegaInt(5).to_bin() == "101"
    assert MegaInt(-255).to_hex() == "-ff"


def test_str_repr():
    assert str(MegaInt(-987654321)) == str(-987654321)
    assert repr(MegaInt(123)) == "MegaInt('123')"
    r = repr(MegaInt(10**90))
    assert r.startswith("MegaInt('10000") and "digits=91" in r


def test_format_minilanguage_parity():
    v = -1234567
    V = MegaInt(v)
    specs = [",d", "_d", "x", "X", "o", "b", ">12,d", "<12", "08d", "+d",
             "^10x", "d", "", "n", "#x", "#o", "#b", "*>20,X", "=+15,d"]
    for spec in specs:
        try:
            ref, ref_err = format(v, spec), None
        except ValueError as e:
            ref, ref_err = None, str(e)
        try:
            got, got_err = format(V, spec), None
        except ValueError as e:
            got, got_err = None, str(e)
        if ref_err is None:
            assert got_err is None and got == ref, (spec, got_err, got, ref)
        else:
            # CPython rejects; MegaNum must reject with the same class.
            assert got_err is not None, (spec, ref_err, got)
    # Grouping with a non-decimal type is rejected like CPython.
    with pytest.raises(ValueError):
        format(V, ",x")


def test_bytes_roundtrip():
    for v, signed in [(0, False), (255, False), (256, False), (-1, True),
                      (-128, True), (2**70 - 5, True), (2**70 - 5, False)]:
        n = max(1, (v.bit_length() + 7 + (8 if signed else 0)) // 8)
        ref = v.to_bytes(n, "little", signed=signed)
        got = MegaInt(v).to_bytes(n, "little", signed=signed)
        assert bytes(got) == ref
        assert int(MegaInt.from_bytes(ref, "little", signed)) == int.from_bytes(
            ref, "little", signed=signed
        )
    with pytest.raises(OverflowError):
        MegaInt(300).to_bytes(1, "big", signed=False)


def test_about_and_version(wheel_about):
    assert wheel_about["version"] == meganum.__version__
    assert "features" in wheel_about and "simd" in wheel_about
