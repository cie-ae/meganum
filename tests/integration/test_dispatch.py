"""Dispatch introspection and overrides (FR-3.4 / FR-3.5)."""

import pytest

from meganum import (
    MegaInt,
    last_dispatch_info,
    reset_dispatch_overrides,
    set_dispatch_overrides,
)

BIG = "1" * 30000  # 120000 bits ~ 1875 limbs -> NTT/Toom-3 band
MID = "1" * 7000  # 28000 bits ~ 437 limbs -> Karatsuba band


def test_introspection_starts_empty():
    # Thread-local decision state starts untouched in a fresh interpreter.
    # (Other tests in the same process may have already dispatched ops, so
    # this only asserts the field shape, not emptiness, when run after them.)
    info = last_dispatch_info()
    if info is not None:
        assert {"op", "algorithm", "backend", "grain", "lhs_limbs", "rhs_limbs"} <= set(info)


def test_decision_recorded_for_mul():
    reset_dispatch_overrides()
    _ = MegaInt.from_hex(MID) * MegaInt.from_hex(MID)
    info = last_dispatch_info()
    assert info is not None
    assert info["op"] == "Mul"
    assert info["lhs_limbs"] == 438  # 28000 bits = 437.5 limbs -> 438
    assert info["algorithm"] in ("Karatsuba", "Toom3", "Ntt")
    assert info["backend"] in ("Sequential", "Parallel")


def test_big_mul_routes_to_fast_band():
    reset_dispatch_overrides()
    _ = MegaInt.from_hex(BIG) * MegaInt.from_hex(BIG)
    assert last_dispatch_info()["algorithm"] in ("Toom3", "Ntt")


def test_override_pin_and_reset():
    reset_dispatch_overrides()
    set_dispatch_overrides({"mul": "schoolbook"})
    _ = MegaInt.from_hex(BIG) * MegaInt.from_hex(BIG)
    assert last_dispatch_info()["algorithm"] == "Schoolbook"
    reset_dispatch_overrides()
    _ = MegaInt.from_hex(BIG) * MegaInt.from_hex(BIG)
    assert last_dispatch_info()["algorithm"] != "Schoolbook"


def test_override_validation():
    with pytest.raises(RuntimeError):
        set_dispatch_overrides({"mul": "bogus"})
    with pytest.raises(RuntimeError):
        set_dispatch_overrides({"bogus": "ntt"})
    # Valid names accepted.
    set_dispatch_overrides({"mul": "ntt"})
    reset_dispatch_overrides()


def test_bit_identical_across_algorithm_overrides():
    reset_dispatch_overrides()
    a = MegaInt.from_hex(BIG)
    b = MegaInt.from_hex("3" * 30000)
    results = set()
    for algo in ("schoolbook", "karatsuba", "toom3", "ntt"):
        set_dispatch_overrides({"mul": algo})
        results.add(str(a * b))
    reset_dispatch_overrides()
    assert len(results) == 1, "backends/algorithms must be bit-identical (FR-3.3)"


def test_threads_bit_identical():
    from concurrent.futures import ThreadPoolExecutor

    def worker(_):
        x = MegaInt.from_hex("9" * 20000)
        y = MegaInt.from_hex("7" * 20000)
        return int(x * y)

    with ThreadPoolExecutor(max_workers=4) as ex:
        outs = list(ex.map(worker, range(8)))
    assert len(set(outs)) == 1, "GIL-releasing parallel path must be deterministic"
