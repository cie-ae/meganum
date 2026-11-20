"""Time the 1,000,000-digit square pipeline and inspect the dispatch.

Builds a 1,000,000-digit operand from a decimal string, squares it, and
formats the result, reporting wall-clock timings measured with
time.perf_counter. Timings are machine-dependent observations, not
assertions. Prints meganum.last_dispatch_info() so you can see the
algorithm, backend, and grain chosen for the square, and verifies the
parse/format round-trip on a smaller 100,000-digit case.

Usage:
    python million_digits.py
"""

import sys
import time

import meganum
from meganum import MegaInt


def timed(fn):
    """Run fn() and return (result, elapsed milliseconds)."""
    start = time.perf_counter()
    result = fn()
    return result, (time.perf_counter() - start) * 1000.0


def main():
    source = "9" * 1_000_000
    value, parse_ms = timed(lambda: MegaInt(source))
    print(f"parsed {len(source)} digits in {parse_ms:.1f} ms")

    square, square_ms = timed(lambda: value * value)
    print(f"squared 1M x 1M digits in {square_ms:.1f} ms")

    text, format_ms = timed(lambda: str(square))
    print(f"formatted to {len(text)} digits in {format_ms:.1f} ms")

    print("last dispatch:", meganum.last_dispatch_info())

    # Round-trip check on a smaller case keeps the demo honest without
    # duplicating the 1M-digit work: parse(str(v)) must reproduce v.
    small = MegaInt("9" * 100_000)
    small_square = small * small
    assert MegaInt(str(small)) == small
    assert MegaInt(str(small_square)) == small_square
    print("round-trip on the 100k-digit operand: "
          "parse(str(v)) == v and parse(str(v*v)) == v*v")
    return 0


if __name__ == "__main__":
    sys.exit(main())
