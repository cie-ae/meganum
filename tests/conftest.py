"""Shared fixtures for the MegaNum pytest suite.

The suite runs against an *installed* meganum wheel (CI: `pip install
dist/*.whl` first, then `pytest tests/`); no build step happens here.
"""

import random

import pytest

import meganum
from meganum import MegaInt


@pytest.fixture(scope="session")
def rng():
    """Deterministic RNG so failures are reproducible."""
    return random.Random(20261119)


@pytest.fixture(scope="session")
def wheel_about():
    return meganum.about()


@pytest.fixture
def megaint_cls():
    return MegaInt


def random_int(rng, bits=None, choices=(16, 64, 200, 700, 3000)):
    bits = bits if bits is not None else rng.choice(choices)
    v = rng.getrandbits(bits)
    return -v if rng.random() < 0.5 else v


def as_native(v):
    """CPython int -> MegaInt (accepts int or MegaInt passthrough)."""
    if isinstance(v, MegaInt):
        return v
    return MegaInt(v)
