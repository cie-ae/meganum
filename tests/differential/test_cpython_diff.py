"""Differential suite: randomized MegaInt vs CPython int (spec 10).

Same generator seeds on every OS -> cross-platform determinism evidence
(S-7). Sizes stay moderate so the full suite runs in seconds in CI.
"""

import random

import pytest

from meganum import MegaInt

OPS_BINARY = ["add", "sub", "mul", "floordiv", "mod", "and", "or", "xor"]
BITS_POOL = [1, 2, 8, 64, 200, 900, 4000]


def rand_val(rng):
    bits = rng.choice(BITS_POOL)
    v = rng.getrandbits(bits)
    return -v if rng.random() < 0.5 else v


@pytest.mark.parametrize("trial", range(150))
def test_binary_ops_differential(trial):
    rng = random.Random(0xC0FFEE + trial)
    a, b = rand_val(rng), rand_val(rng)
    A, B = MegaInt(a), MegaInt(b)
    checks = {
        "add": a + b,
        "sub": a - b,
        "mul": a * b,
        "and": a & b,
        "or": a | b,
        "xor": a ^ b,
    }
    for name, ref in checks.items():
        got = {
            "add": A + B,
            "sub": A - B,
            "mul": A * B,
            "and": A & B,
            "or": A | B,
            "xor": A ^ B,
        }[name]
        assert int(got) == ref, (trial, name, a, b)
    if b != 0:
        assert int(A // B) == a // b, (trial, a, b)
        assert int(A % B) == a % b, (trial, a, b)


@pytest.mark.parametrize("trial", range(40))
def test_shift_differential(trial):
    rng = random.Random(0xB105 + trial)
    a = rand_val(rng)
    s = rng.randrange(0, 5000)
    A = MegaInt(a)
    assert int(A << s) == a << s
    assert int(A >> s) == a >> s


@pytest.mark.parametrize("trial", range(30))
def test_modpow_differential(trial):
    rng = random.Random(0x7070 + trial)
    a = rand_val(rng)
    e = rng.randrange(0, 500)
    m = rand_val(rng) or 1
    A, E, M = MegaInt(a), MegaInt(e), MegaInt(m)
    if e >= 0:
        assert int(pow(A, E, M)) == pow(a, e, m), (trial, a, e, m)


@pytest.mark.parametrize("trial", range(30))
def test_str_roundtrip_differential(trial):
    rng = random.Random(0x5721 + trial)
    v = rand_val(rng)
    assert int(MegaInt(str(v))) == v
    assert str(MegaInt(v)) == str(v)
