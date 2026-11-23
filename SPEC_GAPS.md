# SPEC_GAPS.md — decision log for spec gaps

One line per decision made where the spec was silent or the environment
forced a choice (spec section 0 protocol). Newest last per section.

## Environment constraints

- Sandbox has 2 vCPUs / 4 GB RAM: spec reference hardware is 8 cores / 32 GB.
  All performance numbers recorded locally are labeled with this hardware;
  the gated budgets (P1-P8, P12, P14) are verified on CI runner classes, and
  P14 (4-thread scaling) cannot be validated in the 2-vCPU sandbox at all.
- PyPI publishing, GitHub Actions runs, the 24 h fuzz soak, ASAN/TSAN nightly
  and the cross-OS determinism matrix require a real GitHub repository and
  PyPI project ownership. All artifacts are committed ready-to-run
  (`.github/workflows/*`, release runbook); execution is listed as pending
  in the release checklist rather than claimed done.
- The PyPI project name `meganum` cannot be reserved from this environment;
  the release runbook lists it as a pre-release manual step.

## API interpretation

- Spec 8.3 shows `format(y, precision=50, notation="sci")`; builtins
  `format()` cannot take keyword arguments. Implemented as the method
  `MegaFloat.format(precision=..., notation=...)` (ADR-0005 companion
  note); docs show the method form.
- `MegaInt.__truediv__` raises `TypeError` pointing to `MegaFloat` (ADR-0005);
  spec 8.2 operator list excludes `/` consistently with this.
- `MegaInt.leading_zeros()` (FR-1.3) is defined as
  `64 * limb_count - bit_length` (zero-padded to limb boundary); CPython has
  no such method, so the definition is documented in docstrings and docs.
- `ConversionError` has bases `(ValueError, OverflowError)` per 8.4; the
  native type is created subclassing `ValueError` and the Python shim adds
  `OverflowError` as a second base, with the concrete type registered back
  into the native module so Rust raises the documented class.
- pyo3 0.30 removed `prepare_freethreaded_python()`; for extension modules
  the interpreter is guaranteed initialized before the module function runs,
  which satisfies the spec 5.7 contract (documented in ADR-0001).
- pyo3 0.30 renamed `with_gil`/`allow_threads` to `attach`/`detach`; binding
  code uses the 0.30 API.

## Core engine

- `rust-toolchain.toml` pins `channel = "stable"` for dev convenience; MSRV
  1.80 is enforced by `rust-version` and the dedicated CI matrix job
  (smallest workable default; a hard pin would block local builds on
  toolchains newer/older than one exact version).
- M1 ships Newton reciprocal division (shared primitive with sqrt/MegaFloat
  per spec 6) alongside Knuth schoolbook division; Burnikel-Ziegler lands at
  M2 as the large-divmod primary path per spec 16.
- isqrt at M1 uses full-precision Newton with exact floor correction; the
  M2 upgrade switches the inner loop to precision doubling (planned work,
  tracked here so the interim cost is visible).
- Base-62 alphabet: digits 0-9, a-z (10-35), A-Z (36-61); for radix <= 36
  both letter cases are accepted with values 10-35 (CPython parity: CPython
  treats letters case-insensitively up to its max base 36), for radix > 36
  uppercase maps to 36-61. Documented in `docs/user-guide.md`.
- Exception hierarchy wiring (spec 8.4): Rust can give each native
  exception only ONE base, but every leaf must sit under both its Pythonic
  base and the MeganumError root. The shim creates dual-base subclasses
  (`class ParseError(NativeParseError, MeganumError)`, ...) and registers
  them back via `_register_error_classes`; Rust raises the registered
  classes, native single-base types remain as fallbacks for raw embedders.
- `pow(x, y, 0)` maps to `ParseError` (CPython raises `ValueError` for a
  zero modulus in `pow`, unlike division where `ZeroDivisionError` applies).
- 3-arg `pow` with a negative exponent computes the modular inverse
  (exact CPython semantics); the `TypeError` on negative exponents applies
  only to 2-arg `**`, where CPython would escape to float (ADR-0005
  companion).
- Tunables storage: `tunables.toml` embedded via `include_str!` and parsed by
  a purpose-built strict-subset parser (ADR-0004) - no external TOML crate,
  per the spec 2 dependency budget.

## Toolchain and MSRV (2026-11-19)

- MSRV enforcement is core-only under rustc 1.80 (ADR-0006): pyo3 0.30
  requires rustc >= 1.85 and the dev-dependency ecosystem has drifted past
  1.80, so the old full-workspace 1.80 matrix legs could never pass.
  `test-workspace` (stable, 3 OS, --locked) + `test-msrv-core` (3 OS,
  rust-version-fallback resolution + rustc 1.80 via RUSTC/RUSTDOC
  override) replace it. simd-avx512 stays excluded from the MSRV leg (its
  intrinsics stabilized in Rust 1.89; ADR-0002).
- Dev-dependency floors widened (`proptest = "1"`, `criterion = "0.5"`) so
  the MSRV fallback resolver can select 1.80-compatible versions; the
  committed lock pins criterion 0.5.1 / proptest 1.12.0 for stable legs.

## Packaging

- `pyproject.toml` uses `[tool.maturin] manifest-path` (maturin 1.16 key) to
  point at `crates/meganum-python/Cargo.toml` inside the workspace; version
  is `dynamic = ["version"]` and single-sourced from that Cargo.toml per
  spec 12.1.
- Pass-through cargo features `parallel`/`simd` are declared on
  `meganum-python` so the maturin feature list activates engine features.

## CI hardening (first green-run fixes, 2026-11-19)

- rustdoc `-D warnings`: the crate-level doc linked `[`context::Context`]`
  and `[`_ref`]`, neither of which exists yet (`context.rs` is a placeholder
  until the M4 wave; `_ref` is cfg(test/feature)-gated). Links were reworded
  to plain code spans; the intra-doc links come back with the types in M4.
- Coverage gate script: lcov branch lines use 4-char prefixes (`BRF:`/`BRH:`),
  the aggregator sliced them with `line[3:]` and crashed on `int(':0')` —
  the branch gate has been fixed to `line[4:]` (it now actually runs).
- pytest collection: `tests/` became a package (`__init__.py` in `tests/`,
  `tests/integration/`, `tests/differential/`) + `pythonpath = ["."]` in
  `[tool.pytest.ini_options]`, so `from tests.conftest import ...` resolves
  under pytest's prepend import mode from any working directory.
- Wheel smoke tests inside pypa containers: manylinux images ship no
  `python3` on PATH and musllinux's `/usr/bin/python3` has no pip, so the
  smoke step now discovers a `/opt/python/cp3XX-cp3XX` interpreter >= 3.10
  (abi3-py310 wheels load on any CPython >= 3.10) in ci.yml and release.yml.
- bench-gate: a base commit that predates the Cargo workspace (the bare
  "Initial commit" of a bootstrapped repo) has no baseline to measure; the
  job records `BASE_HAS_WORKSPACE=false` and skips the comparison with a
  repo-owner-visible warning instead of failing. Every real baseline-vs-PR
  comparison path is unchanged.
- cargo-deny: pinned to 0.21.0 in ci.yml/nightly.yml and `deny.toml` was
  migrated to the 0.19+ advisories schema (ADR-0007). Vulnerability errors
  are strictly stronger than the old config; unmaintained crates are
  surfaced by `cargo audit` (non-blocking per spec, no warn level exists
  anymore); yanked stays `warn`.

## Coverage gate reality check (2026-11-19)

- Once the lcov aggregator bug was fixed, the gate ran against real data
  and failed in two structural ways, both now fixed (no thresholds were
  weakened):
  - `bindings` (crates/meganum-python) read 0% because cargo-llvm-cov only
    instruments cargo-run test binaries, while the FFI layer is exercised
    by pytest against an installed wheel. The coverage job now builds an
    instrumented wheel (`RUSTFLAGS="-C instrument-coverage"` via maturin),
    runs pytest with `LLVM_PROFILE_FILE`, and converts the `.so` profile
    with `llvm-profdata` + `llvm-cov export --format=lcov`. The two
    tracefiles are normalized by `scripts/normalize_lcov.py` (the tools
    disagree on `LF:`/`LH:` summaries, so both are rewritten from their
    `DA:` entries) and concatenated before the threshold gate. Measured
    (2026-11-19, Linux x86_64): core 91.6% lines (>= 90 required),
    bindings 96.2% (>= 85 required).
  - `core` was 89.7% because `algos::gcd` (72 lines) had zero Rust-side
    coverage — it was only reachable through Python. Added differential
    tests (u128 Euclid reference for small operands; cross-path agreement
    + divisibility + homogeneity for 36-80 limb operands).
- cargo-llvm-cov emits no `BRDA`/`BRF`/`BRH` records with this LLVM, so
  the branch gate prints its designed skip warning; line gates are live.
- To reach the gates honestly, the differential suite exposed and fixed
  three user-visible bindings bugs (recorded in CHANGELOG [Unreleased]):
  MegaInt-vs-float comparisons truncated the fractional part
  (`5 == 5.5` was `True`); the format mini-language used left as numeric
  default alignment and mishandled the `0` flag with grouping; and
  `round(MegaInt)` without ndigits raised `TypeError` (PyO3 0.30 requires
  an explicit `signature = (ndigits = None)`).

## Second CI-fix pass — latest-stable toolchain (2026-11-19)

Four failures in the attached CI log (`ci-errors.txt`), fixed as follows:

- **macOS link failure** (`Undefined symbols for architecture arm64:
  _Py*` while linking `meganum-python (lib)`): `pyo3/extension-module`
  was enabled unconditionally on `meganum-python`, so every cargo-linked
  binary was built without libpython. Fixed by making it an opt-in crate
  feature that only `pyproject.toml` enables for wheel builds
  (ADR-0009). Plain cargo build/test/clippy now run without it.
- **rustc-1.80 MSRV legs** (`criterion@0.8.2 requires rustc 1.86`, twice):
  the fallback-resolution MSRV job from ADR-0006 did not constrain the
  graph on real runners. Per maintainer directive the 1.80 gate is
  REMOVED entirely (ADR-0008 supersedes ADR-0006): `rust-toolchain.toml`
  tracks `stable` (1.100.0 at time of writing), workspace
  `rust-version` floor 1.85 (PyO3 0.30), `clippy.toml` msrv 1.85, dev
  tree floor 1.88 (proptest 1.12) documented. Dev-dep floors stay broad;
  the committed lock still pins criterion 0.5.1 / proptest 1.12.0.
- **bench-gate** (`Unrecognized option: 'save-baseline'`): the job ran
  `cargo bench -p meganum-core --lib -- --save-baseline`; `--lib` selects
  the libtest harness on the lib unittests, which rejects criterion
  flags. The job now targets the criterion bench: `cargo bench
  -p meganum-core --bench core_ops -- --save-baseline pr`. criterion
  0.5.1 supports `--save-baseline` unconditionally (verified in
  criterion-0.5.1/src/lib.rs), so the dev-dependency set is unchanged.
  Cross-commit baseline comparison stays deferred until a baseline
  exists on main; the job records and uploads the `pr` baseline.
- **Missing workflows in this snapshot**: the repository snapshot this
  pass worked from contains no `.github/` directory, so
  `.github/workflows/ci.yml` is (re)created as the canonical CI
  definition: fmt / clippy (3 OS) / test-core (3 OS) / bench-gate /
  wheel+pytest (3 OS) / supply-chain (cargo-deny@0.21.0 + cargo-audit).
  Actions are pinned to major version tags; commit-SHA pinning stays
  deferred to the M6 hardening pass. If the live repository carries
  additional jobs (coverage, nightly, release), keep them and merge:
  the jobs here replace exactly the ones that failed in the log.
- Direct `cargo build/test` of `meganum-python` is intentionally absent
  from CI: without `extension-module` those binaries link libpython,
  which runner images do not guarantee. Binding correctness is covered
  by the pytest differential suite against an installed wheel on all
  three OSes (587-test suite + `scripts/smoke_api.py`).
- The suite against the wheel immediately caught a real engine bug
  (Newton division out-of-envelope panic, see CHANGELOG) — which is why
  the wheel job pins `--debug` explicitly (maturin 1.16 defaults `build`
  to the dev profile; older maturin defaulted to release, so the flag
  makes CI deterministic and keeps debug assertions live in CI). Release
  wheels ship from release.yml at M7.
- Verified locally on rustc 1.100.0 (all commands are the CI job
  commands): `cargo fmt --all -- --check`; `cargo clippy --workspace
  --all-targets --locked -- -D warnings` (one new-stable lint,
  `manual_repeat_n`, fixed); `cargo test -p meganum-core --locked`
  (default + `parallel,simd`); `cargo bench -p meganum-core --bench
  core_ops -- --save-baseline pr`; maturin wheel build (`cp310-abi3`)
  + install + `pytest tests` (587 passed, 13.3 s) + `scripts/smoke_api.py`
  (SMOKE OK); `cargo deny check advisories bans licenses sources`
  (cargo-deny 0.21.0: all ok). The installed `_native.abi3.so` links no
  libpython (verified with ldd) — the extension-module gating works.
  `cargo audit` is CI-only (fresh RustSec DB per run).
