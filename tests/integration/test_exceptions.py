"""Exception hierarchy (spec 8.4): every leaf sits under both its Pythonic
base and the MeganumError root; Rust raises the registered dual-base classes.
"""

import pytest

import meganum
from meganum import (
    ConversionError,
    DispatchError,
    DivisionByZeroError,
    MeganumError,
    MegaInt,
    ParseError,
    PrecisionError,
    reset_dispatch_overrides,
    set_dispatch_overrides,
)


def test_roots_and_pythonic_bases():
    assert issubclass(ParseError, MeganumError) and issubclass(ParseError, ValueError)
    assert issubclass(DivisionByZeroError, MeganumError) and issubclass(
        DivisionByZeroError, ZeroDivisionError
    )
    assert issubclass(PrecisionError, MeganumError) and issubclass(PrecisionError, ValueError)
    assert issubclass(DispatchError, MeganumError) and issubclass(DispatchError, RuntimeError)
    assert issubclass(ConversionError, MeganumError)
    assert issubclass(ConversionError, ValueError)
    assert issubclass(ConversionError, OverflowError)
    assert issubclass(MeganumError, Exception)


def test_division_by_zero():
    with pytest.raises(DivisionByZeroError):
        MegaInt(1) // MegaInt(0)
    with pytest.raises(DivisionByZeroError):
        MegaInt(1) % MegaInt(0)
    with pytest.raises(ZeroDivisionError):
        MegaInt(1) // MegaInt(0)
    with pytest.raises(MeganumError):
        MegaInt(1) // MegaInt(0)


def test_parse_error():
    with pytest.raises(ParseError):
        MegaInt.parse("nope")
    with pytest.raises(ValueError):
        MegaInt.parse("nope")


def test_conversion_error_is_overflow_and_value():
    with pytest.raises(ConversionError):
        MegaInt(300).to_bytes(1, "big", signed=False)
    with pytest.raises(OverflowError):
        MegaInt(300).to_bytes(1, "big", signed=False)


def test_precision_error():
    with pytest.raises(PrecisionError):
        MegaInt(2**64) ** (2**26)


def test_dispatch_error():
    reset_dispatch_overrides()
    with pytest.raises(DispatchError):
        set_dispatch_overrides({"mul": "bogus"})
    with pytest.raises(RuntimeError):
        set_dispatch_overrides({"bogus_op": "ntt"})
    reset_dispatch_overrides()


def test_typeerror_for_non_numeric_operands():
    with pytest.raises(TypeError):
        MegaInt(1) * "x"
    with pytest.raises(TypeError):
        MegaInt(1) + "x"
