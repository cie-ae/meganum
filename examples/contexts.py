"""Demonstrate MegaFloat contexts, rounding modes, and int() truncation.

Precision is expressed in bits: 50 and 100 decimal digits correspond to 167
and 333 bits (ceil(digits * log2(10))). The script computes pi and sqrt(2)
at those precisions, shows that nested contexts resolve to the innermost
active one and that the outer environment resumes when the inner block
exits, renders 1/3 at a deliberately small precision under all six rounding
modes, and shows that int() truncates toward zero.

Usage:
    python contexts.py
"""

import math
import sys

from meganum import Context, MegaFloat, Rounding

ROUNDING_MODES = (
    Rounding.NEAREST_EVEN,
    Rounding.NEAREST_AWAY,
    Rounding.FLOOR,
    Rounding.CEILING,
    Rounding.UP,
    Rounding.DOWN,
)


def bits_for_digits(digits):
    """Convert a decimal digit count to a bit precision."""
    return math.ceil(digits * math.log2(10))


def main():
    b50 = bits_for_digits(50)
    b100 = bits_for_digits(100)
    print(f"50 decimal digits -> {b50} bits; "
          f"100 decimal digits -> {b100} bits")

    # pi and sqrt(2) at the two precisions. MegaFloat.pi() is correctly
    # rounded on demand at the active precision and cached per environment.
    with Context(precision=b50, rounding=Rounding.NEAREST_EVEN):
        pi50 = MegaFloat.pi()
        outer = MegaFloat(2).sqrt()
    with Context(precision=b100, rounding=Rounding.NEAREST_EVEN):
        inner = MegaFloat(2).sqrt()
    print("pi at 50-digit precision:       ", str(pi50))
    print("sqrt(2) at 50-digit precision:  ", str(outer))
    print("sqrt(2) at 100-digit precision: ", str(inner))

    # Nesting: the innermost context wins inside its block, and the outer
    # environment resumes once the block exits.
    with Context(precision=b50, rounding=Rounding.NEAREST_EVEN):
        with Context(precision=b100, rounding=Rounding.NEAREST_EVEN):
            inside = MegaFloat(2).sqrt()
        after = MegaFloat(2).sqrt()
    print("innermost context wins inside the block:",
          str(inside) == str(inner))
    print("outer context resumes after the block:  ",
          str(after) == str(outer))

    # Rounding modes: at 8 bits the direction of each mode is visible on
    # 1/3, whose representation cannot hit a boundary exactly.
    for mode in ROUNDING_MODES:
        with Context(precision=8, rounding=mode):
            third = MegaFloat(1) / MegaFloat(3)
        print(f"1/3 at 8 bits, {mode}: {str(third)}")

    # int() truncates toward zero, including for negative values.
    print("int(MegaFloat('3.99')): ", int(MegaFloat("3.99")))
    print("int(MegaFloat('-3.99')):", int(MegaFloat("-3.99")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
