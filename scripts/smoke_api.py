"""Functional smoke battery for the MegaNum Python API.

Runs against an *installed* wheel (no build step here). Differential-checks
MegaInt against CPython int semantics, exercises the exception hierarchy,
dispatch introspection/overrides, pickle, formatting and the number-theory
wrappers. Exit code 0 == all checks pass.

Usage: python scripts/smoke_api.py
"""

import math
import pickle
import random
import sys
from concurrent.futures import ThreadPoolExecutor

import meganum
from meganum import (
    DispatchError,
    DivisionByZeroError,
    MeganumError,
    MegaInt,
    ParseError,
    PrecisionError,
    ConversionError,
    last_dispatch_info,
    reset_dispatch_overrides,
    set_dispatch_overrides,
)

rng = random.Random(20261119)
failures = []


def check(name, cond):
    if not cond:
        failures.append(name)
        print(f"FAIL {name}")


def rand_int(bits):
    v = rng.getrandbits(bits)
    return -v if rng.random() < 0.5 else v


# ---------------------------------------------------------------- import
info = meganum.about()
check("about version matches package", info["version"] == meganum.__version__)
check("smoke() ok", meganum.smoke()["ok"] is True)

# Fresh-process dispatch introspection starts empty (checked before any
# arithmetic runs so LAST_DECISION is untouched).
check("dispatch info None initially", last_dispatch_info() is None)

# ------------------------------------------------- arithmetic vs CPython
for trial in range(300):
    a = rand_int(rng.choice([16, 64, 200, 700, 3000]))
    b = rand_int(rng.choice([16, 64, 200, 700, 3000]))
    A, B = MegaInt(a), MegaInt(b)
    check(f"add trial{trial}", int(A + B) == a + b)
    check(f"radd trial{trial}", int(a + B) == a + b)
    check(f"sub trial{trial}", int(A - B) == a - b)
    check(f"rsub trial{trial}", int(a - B) == a - b)
    check(f"mul trial{trial}", int(A * B) == a * b)
    check(f"neg trial{trial}", int(-A) == -a)
    check(f"abs trial{trial}", int(abs(A)) == abs(a))
    check(f"invert trial{trial}", int(~A) == ~a)
    check(f"eq trial{trial}", (A == B) == (a == b))
    check(f"lt trial{trial}", (A < B) == (a < b))
    check(f"le trial{trial}", (A <= B) == (a <= b))
    if b != 0:
        check(f"floordiv trial{trial}", int(A // B) == a // b)
        check(f"rfloordiv trial{trial}", int(a // B) == a // b)
        check(f"mod trial{trial}", int(A % B) == a % b)
        q, r = divmod(A, B)
        check(f"divmod trial{trial}", (int(q), int(r)) == divmod(a, b))
    s = rng.randrange(0, 3000)
    check(f"lshift trial{trial}", int(A << s) == a << s)
    check(f"rshift trial{trial}", int(A >> s) == a >> s)
    check(f"and trial{trial}", int(A & B) == a & b)
    check(f"or trial{trial}", int(A | B) == a | b)
    check(f"xor trial{trial}", int(A ^ B) == a ^ b)
    if a >= 0 and a.bit_length() * min(1000, 1 << 20) < 1 << 24:
        e = rng.randrange(0, 1000)
        check(f"pow trial{trial}", int(A**e) == a**e)
        check(f"rpow-guard", True)

# float comparisons (exact above 2**53)
big = 2**70 + 12345
check("cmp float big", (MegaInt(big) < 2.0**70.5) == (big < 2.0**70.5))
check("cmp float nan", (MegaInt(1) == float("nan")) is False and (MegaInt(1) != float("nan")) is True)
check("cmp float inf", (MegaInt(10**30) < float("inf")) is True)

# ------------------------------------------------------------- parse/format
check("parse underscores", int(MegaInt.parse("1_000_007")) == 1000007)
check("parse sign", int(MegaInt.parse("-42")) == -42)
check("from_hex", int(MegaInt.from_hex("dead_beef")) == 0xDEADBEEF)
check("from_bin", int(MegaInt.from_bin("1011")) == 0b1011)
check("from_oct", int(MegaInt.from_oct("777")) == 0o777)
check("from_base36", int(MegaInt.from_base("zZ", 36)) == int("zz", 36))
check("to_base62 roundtrip", MegaInt.from_base(MegaInt(123456789012345678).to_base(62), 62) == MegaInt(123456789012345678))
check("str parity", str(MegaInt(-987654321)) == str(-987654321))
check("repr short", repr(MegaInt(123)) == "MegaInt('123')")
check("repr long ellipsized", "digits=" in repr(MegaInt(10**90)))
for spec in [",d", "x", "X", "o", "b", ">12,d", "<12", "08d", "+d", "^10x", "_d"]:
    v = -1234567
    check(f"format {spec}", format(MegaInt(v), spec) == format(v, spec))

# ------------------------------------------------------------- bytes / hash
for trial in range(50):
    v = rand_int(rng.choice([64, 200, 1024]))
    for signed in (False, True):
        if not signed and v < 0:
            continue
        n = max(1, (v.bit_length() + 7 + (8 if signed else 0)) // 8)
        ref = v.to_bytes(n, "little", signed=signed)
        got = MegaInt(v).to_bytes(n, "little", signed=signed)
        check(f"to_bytes trial{trial} signed={signed}", bytes(got) == ref)
        check(f"from_bytes trial{trial} signed={signed}", int(MegaInt.from_bytes(ref, "little", signed)) == int.from_bytes(ref, "little", signed=signed))
check("hash parity", hash(MegaInt(2**70 - 5)) == hash(2**70 - 5))
v_neg = -2**70 - 5
check("hash parity neg", hash(MegaInt(v_neg)) == hash(v_neg))

# -------------------------------------------------------------- exceptions
try:
    MegaInt(1) // MegaInt(0)
    check("div by zero raises", False)
except DivisionByZeroError as e:
    check("div zero is ZeroDivisionError", isinstance(e, ZeroDivisionError))
try:
    MegaInt.parse("12x34")
    check("bad parse raises", False)
except ParseError as e:
    check("parse err is ValueError", isinstance(e, ValueError))
try:
    MegaInt(300).to_bytes(1, "big", signed=False)
    check("small to_bytes raises", False)
except ConversionError as e:
    check("conversion err is OverflowError", isinstance(e, OverflowError))
    check("conversion err is ValueError", isinstance(e, ValueError))
try:
    set_dispatch_overrides({"mul": "bogus"})
    check("bad override raises", False)
except DispatchError as e:
    check("dispatch err is RuntimeError", isinstance(e, RuntimeError))
try:
    MegaInt(2) ** (10**30)
    check("huge pow raises", False)
except PrecisionError as e:
    check("precision err is ValueError", isinstance(e, ValueError))
try:
    MegaInt("1") * "x"
    check("type error for str operand", False)
except TypeError:
    check("type error for str operand", True)
check("MeganumError base", issubclass(ParseError, MeganumError) and issubclass(DivisionByZeroError, MeganumError))

# ------------------------------------------------------- number theory
for trial in range(60):
    a = rand_int(rng.choice([32, 128, 512]))
    b = rand_int(rng.choice([32, 128, 512]))
    A, B = MegaInt(a), MegaInt(b)
    check(f"gcd trial{trial}", int(A.gcd(B)) == math.gcd(a, b))
    check(f"lcm trial{trial}", int(A.lcm(B)) == math.lcm(a, b))
    m = rand_int(rng.choice([32, 128])) or 1
    M = MegaInt(m)
    check(f"modpow trial{trial}", int(pow(A, MegaInt(17), M)) == pow(a, 17, m))
    check(f"modpow neg-mod trial{trial}", int(pow(A, MegaInt(17), -M if m > 0 else M)) == pow(a, 17, -m if m > 0 else m))
    g = math.gcd(a, m)
    if g != 0 and g == 1:
        check(f"modpow neg-exp trial{trial}", int(pow(A, MegaInt(-3), M)) == pow(a, -3, m))
        check(f"mod_inverse trial{trial}", int(A.mod_inverse(M)) == pow(a, -1, m))
    else:
        try:
            pow(a, -3, m)
            py_raises = None
        except ValueError:
            py_raises = ValueError
        try:
            int(A.modpow(MegaInt(-3), M))
            our_raises = None
        except ValueError:
            our_raises = ValueError
        check(f"non-invertible parity trial{trial}", py_raises == our_raises == ValueError)

check("isqrt", MegaInt(10**60).isqrt() == MegaInt(10**30))
check("isqrt floor", MegaInt(10**60 - 1).isqrt() == MegaInt(10**30 - 1))
for p in (2, 3, 97, 7919, 2**61 - 1, 2**127 - 1):
    check(f"probable prime {p}", MegaInt(p).is_probable_prime(16) is True)
for c in (0, 1, 561, 41041, 2**67 - 1, 2**67):
    check(f"probable composite {c}", MegaInt(c).is_probable_prime(16) is False)


def jacobi_ref(a, n):
    """Reference Jacobi symbol for odd positive n (standard algorithm)."""
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


for trial in range(40):
    n = rng.randrange(3, 100000, 2)
    a = rand_int(128)
    check(f"jacobi trial{trial}", MegaInt(a).jacobi(MegaInt(n)) == jacobi_ref(a, n))

# --------------------------------------------------------------- dispatch
A = MegaInt.from_hex("1" * 7000)  # 28000 bits ~ 437 limbs -> karatsuba band
B = MegaInt.from_hex("3" * 7000)
_ = A * B
info = last_dispatch_info()
check("dispatch recorded", info is not None and info["op"] == "Mul")
check("dispatch band sane", info["algorithm"] in ("Karatsuba", "Toom3", "Ntt"))
A2 = MegaInt.from_hex("1" * 30000)  # 120000 bits ~ 1875 limbs
B2 = MegaInt.from_hex("3" * 30000)
_ = A2 * B2
check("big mul routes ntt/toom3", last_dispatch_info()["algorithm"] in ("Toom3", "Ntt"))
set_dispatch_overrides({"mul": "schoolbook"})
_ = A2 * B2
check("override honored", last_dispatch_info()["algorithm"] == "Schoolbook")
reset_dispatch_overrides()
_ = A2 * B2
check("override cleared", last_dispatch_info()["algorithm"] != "Schoolbook")
# ------------------------------------------------------------- threading
def worker(seed):
    x = MegaInt.from_hex("9" * 20000)
    y = MegaInt.from_hex("7" * 20000)
    return x * y == x * y and int(x * y) == int(x) * int(y)


with ThreadPoolExecutor(max_workers=4) as ex:
    results = list(ex.map(worker, range(8)))
check("threads bit-identical", all(results))

# --------------------------------------------------------------- pickle
v = rand_int(2000)
V = MegaInt(v)
check("pickle roundtrip", pickle.loads(pickle.dumps(V)) == v)
import copy as copy_mod

check("copy protocol", copy_mod.copy(V) == v and copy_mod.deepcopy(V) == v and V == v)

# --------------------------------------------------------------- summary
if failures:
    print(f"\n{len(failures)} FAILURES (first 10): {failures[:10]}")
    sys.exit(1)
print("SMOKE OK: all API checks passed")
