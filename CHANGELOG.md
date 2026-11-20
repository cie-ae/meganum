# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **float comparisons** (bindings): `MegaInt` vs `float` comparisons
  truncated the fractional part of the float (`5 == 5.5` was `True`,
  `5 < 5.5` was `False`, `0 < inf` was `False`). The float is now
  decomposed into its exact IEEE-754 value `sig * 2^e` and compared
  exactly (`|a| * 2^-e` against `sig` when the exponent is negative) —
  bit-exact CPython parity, verified by a 180+-case differential suite.
- **format mini-language** (bindings): the numeric default alignment is
  `>` (was `<`); the bare `0` flag defaults the alignment to `=` and the
  fill to `0` while only overriding the fill next to an explicit
  alignment (`'<08d'` -> `'42000000'`, `'=+07d'` -> `'+000007'`);
  zero fill with grouping pads the raw digits to the smallest count whose
  grouped form reaches the width and then inserts separators
  (`format(1234567, '012,d')` -> `'0,001,234,567'`); `#` prefixes apply
  for zero as well (`format(0, '#x')` -> `'0x0'`). All 60+ spec cases are
  differential against CPython `format(int, spec)`.
- **`round(MegaInt)` without ndigits** raised `TypeError` (PyO3 0.30
  requires an explicit `#[pyo3(signature = (ndigits = None))]` for
  optional arguments); `round(x)` and `round(x, n)` both work now.
- **modpow / mod_inverse correctness** (core): `divmod_best` returns
  `(quotient, remainder)`; nine call sites across `algos/pow.rs`,
  `algos/number_theory.rs` and `BigInt::modpow`/`BigInt::mod_inverse`
  destructured the quotient where the remainder was meant. Every
  modular exponentiation and inverse with `base >= modulus` returned a
  wrong value (typically 0) or a spurious "not invertible". Fixed with a
  quotient/remainder audit of all 30+ call sites and new differential
  tests for both the Montgomery (odd modulus) and plain (even modulus)
  paths across 1-3 limb moduli.
- Miller-Rabin witness loop reduced `x^2` by the quotient instead of the
  remainder; probable-prime verdicts beyond the first squaring were
  unreliable.
- Jacobi/probable-prime pseudorandom base generation reduced by the
  quotient instead of the remainder.
- Bindings: `MegaInt.__int__` passed a positional `signed` flag to
  `int.from_bytes` (keyword-only) — every `int(...)` conversion crashed.
- Bindings: `from_base`/`from_hex`/`from_bin`/`from_oct` rejected CPython
  digit-grouping underscores; they now delegate to the core parser (sign,
  prefixes, underscores, radix-parity rules).
- Radix boundary: letters are case-insensitive up to radix 36 (CPython
  parity); the old boundary at radix 35 rejected `int('zZ', 36)`-style
  input.
- Bindings: format mini-language rewritten to CPython order — grouping
  (`,`/`_`) comes AFTER width, `#`/`0`/`=` are supported, grouping with a
  non-decimal type raises like CPython.
- Bindings: `MegaInt.__pow__`'s result-size guard multiplied by the
  exponent's *bit length* instead of its *value*, letting ~536 MB results
  through; 3-arg pow now honors negative exponents via the modular
  inverse (CPython semantics), and the guard uses `bit_length * e`.
- Bindings: `modpow`/`mod_inverse` reduce signed bases into `[0, |m|)`
  first (`pow(-2, 3, 5) == 2`) and map negative moduli onto the modulus's
  sign domain (`pow(2, 3, -3) == -1`), matching CPython exactly.
- Dispatcher overrides no longer leak memory per `set_dispatch_overrides`
  call (`Box::leak` keys replaced with owned strings).
- `MegaInt` comparisons/downcasts ported to the PyO3 0.30 `cast` API.
- **CI: macOS link failure** (`Undefined symbols for architecture arm64:
  _Py*`): `pyo3/extension-module` was enabled unconditionally on
  `meganum-python`, so every cargo-linked binary was built without
  libpython. It is now an opt-in feature that only `pyproject.toml`
  enables for wheel builds (ADR-0009); plain cargo build/test/clippy run
  without it and link libpython normally.
- **CI: bench-gate ran `cargo bench --lib -- --save-baseline`** — `--lib`
  selects the libtest harness on the lib unittests, which rejects
  criterion flags (`Unrecognized option: 'save-baseline'`). The job now
  targets the criterion bench (`--bench core_ops`), which supports
  `--save-baseline` unconditionally in 0.5.1.
- **CI: rustc-1.80 MSRV legs** (`criterion@0.8.2 requires rustc 1.86`):
  the never-passable 1.80 gate is removed per maintainer directive; the
  toolchain tracks latest stable (1.100.0) and the workspace
  `rust-version` floor is 1.85 (ADR-0008).
- **Newton division out-of-envelope panic** (core): `newton::divmod` fed
  `reciprocal_at` a scale `p = n - m + 2` without enforcing the callee
  contract `p >= m + 2`; any division where the quotient is shorter than
  the divisor (`n < 2m` — reachable via the dispatcher's Newton band with
  gap >= 48 limbs and lhs < 2m, e.g. a large modpow modulus) panicked on
  the reciprocal debug assertion ("reciprocal scale must exceed divisor
  limbs"). `divmod` now downgrades that band to exact schoolbook division
  and a regression test pins the (100, 51)-limb shape. Surfaced by the
  pytest differential suite against a debug wheel
  (`test_modpow_differential[0]`); Burnikel–Ziegler was audited and is
  safe (`prec = 2m + 2` always satisfies the contract).

### Added
- Differential test coverage for the previously Rust-untested `algos::gcd`
  paths (binary GCD and Euclid-with-estimated-quotients): u128 Euclid
  reference on small operands, cross-path agreement plus divisibility and
  homogeneity properties on 36-80 limb operands.
- `tests/integration/test_api_surface.py`: format mini-language, float
  comparisons, rounding, reflected operators, byte conversion, bit
  queries, pickle and power guards — differential vs CPython wherever
  CPython has the same operation (suite grows to 587 tests).
- `scripts/normalize_lcov.py`: normalizes lcov tracefiles for the
  coverage gate (DA-derived LF/LH, needle filtering) so `cargo llvm-cov`
  and `llvm-cov export` outputs can be merged.
- Full Python API surface wired into `meganum._native`: `MegaInt` (every
  dunder + number theory + pickle/copy protocol), the spec 8.4 exception
  hierarchy (dual-base leaf classes registered from the shim: each leaf
  sits under both its Pythonic base and `MeganumError`), and the dispatch
  introspection/override API (`last_dispatch_info`,
  `set_dispatch_overrides`, `reset_dispatch_overrides`).
- pytest suite (`tests/integration/`, `tests/differential/`): operator
  semantics, parse/format/bytes parity, exception hierarchy, dispatch
  introspection + bit-identical backend override checks, number-theory
  differential vs CPython, threaded determinism (295 tests).
- `scripts/smoke_api.py`: standalone post-install smoke battery.

### Changed
- Toolchain policy: track **latest stable Rust** (1.100.0 as of
  2026-11-19); the rustc-1.80 MSRV gate is removed (ADR-0008 supersedes
  ADR-0006). Workspace `rust-version` = 1.85 (PyO3 0.30 floor);
  `clippy.toml` msrv = 1.85; the dev tree needs rustc >= 1.88 (proptest
  1.12) and development/CI run latest stable.
- **CI coverage job measures both buckets honestly**: `meganum-core` via
  `cargo llvm-cov`, and the `meganum-python` FFI layer via an
  instrumented wheel (`RUSTFLAGS=-C instrument-coverage`) exercised by
  pytest with `LLVM_PROFILE_FILE`, converted with `llvm-profdata` /
  `llvm-cov export --format=lcov`. The lcov outputs are normalized
  (`scripts/normalize_lcov.py`) and merged before the threshold gate.
  Measured locally: core 91.6% lines (>= 90 required), bindings 96.2%
  lines (>= 85 required); the bindings bucket previously read 0% because
  pytest-driven coverage was invisible to `cargo llvm-cov`.
- CI/nightly pin `cargo-deny@0.21.0` and `deny.toml` targets that schema
  (ADR-0007); coverage-gate lcov branch prefixes (`BRF:`/`BRH:`) parsed
  correctly; wheel smoke tests discover a `/opt/python` interpreter
  inside pypa containers; the bench-gate skips (with a warning) base
  commits that predate the Cargo workspace; `tests/` is a package so
  `from tests.conftest import ...` collects under pytest.
- MSRV policy (ADR-0006): rustc 1.80 is enforced for `meganum-core` via a
  dedicated `test-msrv-core` job (rust-version-fallback resolution +
  rustc/rustdoc 1.80 override, 3 OS); pyo3 0.30 (MSRV 1.85) keeps the
  bindings on stable. Dev-dependency floors widened so the MSRV resolver
  can pick 1.80-compatible versions; committed lock regenerated
  (criterion 0.5.1).

## [1.0.0] - 2026-11-18

### Added

- **MegaInt**: exact arbitrary-precision signed integers, validated to
  1,000,000 decimal digits.
  - Parsing from strings with optional `0x`/`0o`/`0b` prefixes and `_`
    separators, plus `from_hex`, `from_bin`, `from_oct`, `from_base`
    (radix 2-62), and `from_bytes`.
  - Full operator set (`+ - * // % ** << >> & | ^ ~`, `divmod`, `abs`,
    negation, comparisons) with Python floor semantics; `**` requires a
    non-negative exponent and true division raises `TypeError` pointing to
    `MegaFloat`.
  - Number theory helpers: `gcd`, `lcm`, `modpow`, `mod_inverse`, `jacobi`,
    `is_probable_prime`.
  - Bitwise queries and roots: `bit_length`, `bit_count`, `trailing_zeros`,
    `leading_zeros`, `isqrt`.
  - Conversions and output: exact `str`, `to_hex`, `to_bin`, `to_base`,
    `to_bytes`, exact `int` conversion via the limb path (unaffected by
    `sys.set_int_max_str_digits`), `float` with `OverflowError` beyond
    IEEE-754 range, `bool`, `round`, and a format mini-language subset
    (fill, align, sign, comma grouping).
  - Value semantics: immutability, `hash`/`eq` compatible with `int`,
    truncated `repr` for large values, pickle round-trip (protocol <= 5),
    and copy/deepcopy support.
- **MegaFloat**: arbitrary-precision floating point.
  - `Context(precision=..., rounding=...)` with precision in bits and five
    rounding modes (`NEAREST_EVEN`, `NEAREST_AWAY`, `FLOOR`, `CEILING`,
    `UP`, `DOWN`); thread-local default context of 256 bits with
    `NEAREST_EVEN`; nested contexts with innermost-wins semantics and O(1)
    enter.
  - Constants `MegaFloat.pi()`, `MegaFloat.e()`, `MegaFloat.ln2()`,
    correctly rounded on demand at the active precision and cached per
    `(precision, rounding)`.
  - Correctly rounded `+ - * / sqrt` from exact-integer-based rounding
    decisions; `exp`, `ln`, `log2`, `log10`, `pow` via guard-digit
    computation with Ziv-style recomputation; overflow and underflow follow
    the active rounding mode.
  - Formatting with `precision=` and `notation=` (`auto`, `dec`, `sci`,
    `eng`); `int(f)` truncates toward zero.
- **Adaptive dispatcher** with Sequential, Parallel (rayon work-splitting,
  adaptive grain, deterministic merge), and SIMD (runtime-detected
  AVX2/AVX-512 on x86-64, NEON on aarch64, scalar fallback always compiled)
  backends; memory-aware downgrades; bit-identical results across backends.
- **Dispatch introspection and overrides** from Python:
  `last_dispatch_info()`, `set_dispatch_overrides()`,
  `reset_dispatch_overrides()`, and `DispatchError` for invalid overrides.
- abi3 (`cp310`) wheels covering Python 3.10-3.14 for manylinux2014 and
  manylinux_2_28 on x86_64 and aarch64, musllinux_1_2 on x86_64 and
  aarch64, macOS x86_64 and arm64, and Windows x86_64; Windows aarch64 is
  best-effort.
- Source distribution for platforms outside the wheel matrix.
- `.pyi` stubs for the public Python surface (pyright strict-clean).
- Documentation site (mkdocs-material) with user guide, API references,
  algorithm and dispatcher notes, benchmark methodology, and the ADR series
  (0001-0005).
- Dual licensing under MIT OR Apache-2.0, at the licensee's option.

[Unreleased]: https://github.com/meganum/meganum/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/meganum/meganum/releases/tag/v1.0.0
