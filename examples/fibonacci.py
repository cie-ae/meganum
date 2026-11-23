"""Compute Fibonacci numbers with MegaInt two ways.

An iterative sum built from n MegaInt additions and a divide-and-conquer
fast-doubling implementation built from MegaInt multiplication are both
computed and compared, exercising the dispatcher across the size range of
F(n). Usage: python fibonacci.py [n]; n defaults to 100000 and must be a
non-negative integer. Prints the digit count, bit_length, and the first
and last 10 digits of F(n). Above a conservative limit the iterative
cross-check is skipped so the script stays fast on any machine.
"""

import sys

from meganum import MegaInt

# Above this n, the iterative cross-check would dominate the run time and
# is skipped; fast doubling alone handles large n.
ITERATIVE_LIMIT = 100_000


def fib_iterative(n):
    """Return F(n) using n MegaInt additions (F(0) = 0, F(1) = 1)."""
    a, b = MegaInt(0), MegaInt(1)
    for _ in range(n):
        a, b = b, a + b
    return a


def fib_doubling(n):
    """Return (F(n), F(n+1)): F(2k)=F(k)*(2F(k+1)-F(k)), F(2k+1)=F(k)^2+F(k+1)^2."""
    if n == 0:
        return MegaInt(0), MegaInt(1)
    a, b = fib_doubling(n >> 1)
    c = a * (b * MegaInt(2) - a)
    d = a * a + b * b
    if n & 1:
        return d, c + d
    return c, d


def describe(n, fib):
    """Print size facts and the head/tail digits of F(n)."""
    text = str(fib)
    print(f"F({n}) has {len(text)} digits and {fib.bit_length()} bits")
    print(f"first 10 digits: {text[:10]}")
    print(f"last 10 digits:  {text[-10:]}")


def main():
    if len(sys.argv) > 2 or (len(sys.argv) == 2 and not sys.argv[1].isdigit()):
        print("usage: python fibonacci.py [n]  (n must be a non-negative integer)")
        return 2
    n = int(sys.argv[1]) if len(sys.argv) == 2 else 100_000

    fib = fib_doubling(n)[0]
    describe(n, fib)

    if n <= ITERATIVE_LIMIT:
        print("iterative sum and fast doubling agree:", fib_iterative(n) == fib)
    else:
        print("n exceeds the iterative cross-check limit; fast doubling only")
    return 0


if __name__ == "__main__":
    sys.exit(main())
