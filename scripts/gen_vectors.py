#!/usr/bin/env python3
"""Generate the CPython cross-checked known-answer-test corpus for MegaNum.

Every expected value in the output is computed with CPython's own
arbitrary-precision integers, so the committed corpus is a genuine
differential oracle (spec 6: known-answer vectors cross-checked against
CPython). The corpus is deterministic: same seed, same file.

Outputs:
  tests/vectors/cpython_kat.tsv   consumed by the Rust KAT test
  tests/vectors/manifest.json     golden-vector manifest (updated entries)

Run:  python3 scripts/gen_vectors.py [--big N]
"""
from __future__ import annotations

import hashlib
import json
import random
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TSV_PATH = REPO / "tests" / "vectors" / "cpython_kat.tsv"
MANIFEST_PATH = REPO / "tests" / "vectors" / "manifest.json"

OPS_BINARY = ["add", "sub", "mul", "floordiv", "mod", "and", "or", "xor"]
OPS_UNARY = ["neg", "not", "abs"]


def edge_values() -> list[int]:
    vals = [
        0, 1, -1, 2, -2, 3, 7, 10, 255, 256, 2**31, 2**32, 2**63 - 1, 2**63,
        2**64, 2**64 - 1, -(2**63), 2**127, -(2**127), 10**19, 10**19 - 1,
        10**20, -(10**19), 2**128 - 1, (1 << 64) - (1 << 32),
    ]
    vals += [9 * 10**k + 7 for k in (1, 5, 18, 19, 20, 37, 38)]
    vals += [-v for v in vals if v > 0][:12]
    return vals


def random_values(rng: random.Random, count: int, max_digits: int) -> list[int]:
    out = []
    for _ in range(count):
        digits = rng.randrange(1, max_digits)
        v = rng.randrange(0, 10**digits)
        if rng.random() < 0.3:
            v = -v
        out.append(v)
    return out


def pow_cases(rng: random.Random) -> list[tuple[int, int, int]]:
    cases = []
    for b in [0, 1, -1, 2, -2, 3, -3, 10, 123456789012345678901234567890]:
        for e in [0, 1, 2, 3, 17, 64, 65]:
            cases.append((b, e, b**e))
    for _ in range(40):
        b = rng.randrange(-10**12, 10**12)
        e = rng.randrange(0, 40)
        cases.append((b, e, b**e))
    return cases


def shift_cases(rng: random.Random) -> list[tuple[int, int, int, str]]:
    cases = []
    for a in edge_values()[:14]:
        for k in [0, 1, 7, 63, 64, 65, 100, 130]:
            cases.append((a, k, a << k, "shl"))
            if a >= 0 or (a >> k) == a >> k:  # arithmetic shift is well-defined
                cases.append((a, k, a >> k, "shr"))
    for _ in range(30):
        a = rng.randrange(-(10**30), 10**30)
        k = rng.randrange(0, 200)
        cases.append((a, k, a << k, "shl"))
        cases.append((a, k, a >> k, "shr"))
    return cases


def radix_cases(rng: random.Random) -> list[tuple[int, int, str, str]]:
    """op namespace: to_base with radix in extra column."""
    cases = []
    for a in edge_values()[:16]:
        for radix in (2, 8, 16, 36, 62):
            neg = a < 0
            digits = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
            v = -a if neg else a
            if v == 0:
                s = "0"
            else:
                buf = ""
                while v:
                    buf = digits[v % radix] + buf
                    v //= radix
                s = ("-" if neg else "") + buf
            cases.append((a, radix, s, "to_base"))
    for _ in range(60):
        a = rng.randrange(-(10**40), 10**40)
        radix = rng.choice((2, 8, 16, 36, 62))
        neg = a < 0
        digits = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        v = -a if neg else a
        buf = ""
        while v:
            buf = digits[v % radix] + buf
            v //= radix
        s = ("-" if neg else "") + (buf or "0")
        cases.append((a, radix, s, "to_base"))
    return cases


def divmod_guard(a: int, b: int) -> bool:
    return b != 0


def main() -> int:
    big = 400
    if "--big" in sys.argv:
        big = int(sys.argv[sys.argv.index("--big") + 1])
    rng = random.Random(20261118)
    rows: list[str] = []

    def emit(op: str, a: int, b: int, extra: str, expected: int | str) -> None:
        assert isinstance(expected, int) or isinstance(expected, str)
        rows.append(f"{op}\t{a}\t{b}\t{extra}\t{expected}")

    vals = edge_values() + random_values(rng, 220, big)
    for a in vals:
        for b in [0, 1, -1, 2, -2, 7, -7, 2**64, -(2**64), 10**19]:
            for op in OPS_BINARY:
                if op in ("floordiv", "mod") and not divmod_guard(a, b):
                    continue
                if op == "add":
                    emit(op, a, b, "", a + b)
                elif op == "sub":
                    emit(op, a, b, "", a - b)
                elif op == "mul":
                    emit(op, a, b, "", a * b)
                elif op == "floordiv":
                    emit(op, a, b, "", a // b)
                elif op == "mod":
                    emit(op, a, b, "", a % b)
                elif op == "and":
                    emit(op, a, b, "", a & b)
                elif op == "or":
                    emit(op, a, b, "", a | b)
                elif op == "xor":
                    emit(op, a, b, "", a ^ b)
    for a in vals:
        emit("neg", a, 0, "", -a)
        emit("not", a, 0, "", ~a)
        emit("abs", a, 0, "", abs(a))
    for b, e, r in pow_cases(rng):
        emit("pow", b, e, "", r)
    for a, k, r, op in shift_cases(rng):
        emit(op, a, k, "", r)
    for a, radix, s, op in radix_cases(rng):
        emit(op, a, radix, "", s)

    TSV_PATH.write_text("\n".join(rows) + "\n", encoding="utf-8")
    digest = hashlib.sha256(TSV_PATH.read_bytes()).hexdigest()

    manifest = json.loads(MANIFEST_PATH.read_text())
    manifest["entries"] = manifest.get("entries", [])
    manifest["kat"] = {
        "file": "tests/vectors/cpython_kat.tsv",
        "rows": len(rows),
        "sha256": digest,
        "generator": "scripts/gen_vectors.py",
        "python_checked": True,
    }
    MANIFEST_PATH.write_text(json.dumps(manifest, indent=2) + "\n")

    print(f"wrote {len(rows)} CPython-verified rows to {TSV_PATH}")
    print(f"tsv sha256 = {digest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
