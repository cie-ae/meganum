"""MegaNum: fast, exact, portable arbitrary-precision integers and floats.

Rust engine with adaptive Sequential / Parallel / SIMD execution, exposed
to Python through native wheels. See the docs site for the full guide.
"""

from meganum import _native

__version__ = "1.0.0"

_core_version = _native.core_version
about = _native.about

# Exceptions (spec 8.4). Rust can give each native exception only one
# base, but every leaf must sit under BOTH its Pythonic base and the
# MeganumError root. The shim creates the dual-base subclasses and
# registers them back into the native module so Rust raises exactly the
# documented classes; the native single-base types remain as fallbacks
# for raw embedders that skip the shim.
MeganumError = _native.MeganumError


class ParseError(_native.NativeParseError, MeganumError):
    """Invalid string or bytes input (also a ``ValueError``)."""


class DivisionByZeroError(_native.NativeDivisionByZeroError, MeganumError):
    """Division or modulo by zero (also a ``ZeroDivisionError``)."""


class PrecisionError(_native.NativePrecisionError, MeganumError):
    """Precision request beyond the supported envelope (a ``ValueError``)."""


class DispatchError(_native.NativeDispatchError, MeganumError):
    """Invalid dispatch override configuration (a ``RuntimeError``)."""


class ConversionError(_native.NativeConversionError, MeganumError, OverflowError):
    """Lossy or out-of-range conversion (also an ``OverflowError``)."""


_native._register_error_classes(
    ParseError,
    DivisionByZeroError,
    PrecisionError,
    DispatchError,
    ConversionError,
)

# Core numeric type. MegaFloat / Context / Rounding land with the M4 wave.
MegaInt = _native.MegaInt

# Dispatch introspection and overrides (FR-3.4 / FR-3.5).
last_dispatch_info = _native.last_dispatch_info
set_dispatch_overrides = _native.set_dispatch_overrides
reset_dispatch_overrides = _native.reset_dispatch_overrides

__all__ = [
    "MegaInt",
    "MeganumError",
    "ParseError",
    "DivisionByZeroError",
    "PrecisionError",
    "DispatchError",
    "ConversionError",
    "last_dispatch_info",
    "set_dispatch_overrides",
    "reset_dispatch_overrides",
    "about",
    "smoke",
    "__version__",
]


def smoke() -> dict:
    """Run a fast self-check battery; raises on any failure.

    Returns a summary dict. Used by CI wheel checks and by
    ``scripts/post_install_smoke.py`` (spec section 19-D).

    >>> s = smoke()
    >>> s["ok"]
    True
    """
    import time

    t0 = time.perf_counter()
    info = about()
    import_ms = (time.perf_counter() - t0) * 1000.0
    if info["version"] != __version__:
        raise RuntimeError(
            f"version mismatch: package {__version__} vs native {info['version']}"
        )
    return {"ok": True, "version": __version__, "about": info, "about_ms": import_ms}
