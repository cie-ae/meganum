"""Number-theory wrappers (FR-1.4) vs pure-Python references."""

import math
import random

import pytest

from meganum import MegaInt

M57 = (1 << 61) - 1
M127 = (1 << 127) - 1


def jacobi_ref(a, n):
    """Standard Jacobi symbol for odd positive n."""
    assert n > 0 and n % 2 == 1
    a %= n
    result = 1
    while a != 0:
        while a % 2 == 0:
            a //= 2
            if n % 8 in (3, 5):
                result = -result
        a, n = n, a
        if a % 4 == 3 and n % 4 == 3:
            result = -result
        a %= n
    return result if n == 1 else 0


def test_gcd_lcm_vs_math(rng=None):
    rng = rng or random.Random(20261119)
    for _ in range(60):
        a = rng.randint(-2**512, 2**512)
        b = rng.randint(-2**512, 2**512)
        A, B = MegaInt(a), MegaInt(b)
        assert int(A.gcd(B)) == math.gcd(a, b)
        assert int(A.lcm(B)) == math.lcm(a, b)
    assert int(MegaInt(0).gcd(MegaInt(0))) == 0
    assert int(MegaInt(0).lcm(MegaInt(5))) == 0


def test_isqrt():
    assert MegaInt(10**60).isqrt() == MegaInt(10**30)
    assert MegaInt(10**60 - 1).isqrt() == MegaInt(10**30 - 1)
    with pytest.raises(Exception):
        MegaInt(-4).isqrt()


def test_probable_primes():
    for p in (2, 3, 97, 7919, M57, M127):
        assert MegaInt(p).is_probable_prime(16) is True, p
    for c in (0, 1, 561, 41041, 2**67 - 1, 2**67):
        assert MegaInt(c).is_probable_prime(16) is False, c


def test_modpow_parity_with_cpython():
    rng = random.Random(4242)
    for _ in range(60):
        a = rng.randint(-2**300, 2**300)
        e = rng.randint(0, 200)
        m = rng.randint(-2**300, 2**300) or 1
        A, E, M = MegaInt(a), MegaInt(e), MegaInt(m)
        assert int(A.modpow(E, M)) == pow(a, e, m), (a, e, m)
        assert int(pow(A, E, M)) == pow(a, e, m)


def test_modpow_negative_exponent():
    rng = random.Random(777)
    for _ in range(40):
        a = rng.randint(-2**200, 2**200)
        e = -rng.randint(1, 50)
        m = rng.randint(-2**200, 2**200) or 1
        A, E, M = MegaInt(a), MegaInt(e), MegaInt(m)
        if math.gcd(a, m) == 1:
            assert int(pow(A, E, M)) == pow(a, e, m), (a, e, m)
        else:
            with pytest.raises(ValueError):
                pow(A, E, M)


def test_modpow_zero_modulus_is_valueerror():
    # CPython raises ValueError for pow(x, y, 0).
    with pytest.raises(ValueError):
        pow(MegaInt(2), MegaInt(3), MegaInt(0))
    with pytest.raises(ValueError):
        MegaInt(2).modpow(MegaInt(3), MegaInt(0))


def test_mod_inverse_parity():
    rng = random.Random(99)
    for _ in range(40):
        a = rng.randint(-2**200, 2**200)
        m = rng.randint(-2**200, 2**200) or 1
        A, M = MegaInt(a), MegaInt(m)
        if math.gcd(a, m) == 1:
            assert int(A.mod_inverse(M)) == pow(a, -1, m), (a, m)
        else:
            with pytest.raises(ValueError):
                A.mod_inverse(M)


def test_jacobi_vs_reference():
    rng = random.Random(31337)
    for _ in range(40):
        n = rng.randrange(3, 100000, 2)
        a = rng.randint(-2**128, 2**128)
        assert MegaInt(a).jacobi(MegaInt(n)) == jacobi_ref(a, n), (a, n)
    # Negative a uses the (-1/n) twist; periodicity keeps parity with the
    # reduced reference above. Even/non-positive moduli are rejected.
    with pytest.raises(ValueError):
        MegaInt(3).jacobi(MegaInt(8))
    with pytest.raises(ValueError):
        MegaInt(3).jacobi(MegaInt(0))
    with pytest.raises(ValueError):
        MegaInt(3).jacobi(MegaInt(-7))


def test_pickling_and_copy():
    import copy
    import pickle

    v = 2**500 + 12345
    V = MegaInt(v)
    assert pickle.loads(pickle.dumps(V)) == v
    assert copy.copy(V) == v
    assert copy.deepcopy(V) == v
