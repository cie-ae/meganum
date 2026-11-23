# ADR 0006: MSRV policy — core-only rustc 1.80 gate with MSRV-aware resolution

## Status

Superseded by [ADR-0008](0008-toolchain-latest-stable.md) (2026-11-19:
the rustc-1.80 gate was removed at maintainer request; the toolchain
tracks latest stable). The decision body below is preserved as history.

## Date

2026-11-19

## Context

SPEC pins MSRV 1.80. Two independent developments made a full-workspace
rustc-1.80 gate unachievable without weakening something else:

1. **PyO3 0.30 (ADR-0001) declares MSRV 1.85.** The bindings crate
   (`meganum-python`) therefore cannot be compiled by rustc 1.80 at all.
2. **The dev-dependency ecosystem has drifted past 1.80** (criterion 0.8
   requires 1.86, proptest 1.12 requires 1.88, clap_lex 1.1 requires 1.85),
   so a rustc-1.80 job cannot even resolve the committed lockfile: the
   committed lock serves the stable legs.

The original `test-core` matrix (os x [stable, 1.80] over the whole
workspace) would fail on every 1.80 leg — a gate that can never pass.

## Decision

- **MSRV scope**: rustc 1.80 is enforced for `meganum-core`, the pure-Rust
  engine crate (its runtime dependency surface is thiserror + rayon, both
  1.80-compatible). The spec's MSRV concern — a Rust consumer building the
  engine — is fully preserved. The bindings crate's effective MSRV is 1.85,
  inherited from PyO3 0.30; bindings are fully tested on stable.
- **CI shape**: the old `test-core` matrix splits into
  - `test-workspace` (stable, 3 OS, `--locked`), and
  - `test-msrv-core` (3 OS): fresh resolution with
    `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` (cargo's
    rust-version-aware resolver, guided by the workspace
    `rust-version = "1.80"`), then `cargo test -p meganum-core
    --features parallel,simd` with `RUSTC`/`RUSTDOC` pointed at the 1.80
    toolchain. The committed lockfile is untouched for the stable legs.
- **dev-dependency floors widened** (`proptest = "1"`,
  `criterion = "0.5"`) so the fallback resolver can select 1.80-compatible
  versions. The committed lock pins criterion 0.5.1 / proptest 1.12.0 for
  stable; benches compile against criterion 0.5 (same public API).
- **`simd-avx512` is excluded from the MSRV leg** (already opt-in per
  ADR-0002): the AVX-512 intrinsics stabilized in Rust 1.89.
- **Feature set on the MSRV leg is `parallel,simd`**, not `--all-features`,
  for exactly the same reason.

## Consequences

- The MSRV gate now actually passes while still compiling and running the
  full core test suite (102 lib tests + dispatcher + differential + KAT)
  under rustc 1.80 on all three OSes.
- When PyO3 is bumped to a line with MSRV <= 1.80 (or the MSRV is formally
  raised through a new ADR), the two jobs can merge back into one matrix.
- `cargo update` inside the MSRV job re-resolves on every run; a future
  yank of all 1.80-compatible versions of a runtime dependency would
  surface as an MSRV job failure — which is the correct signal.

## Evidence

- Local verification (2026-11-19): fresh fallback resolution + rustc 1.80
  compiles and passes the full `meganum-core` suite (102 + 6 + 22 + 3
  tests, including doctests) on Linux x86_64.
- `cargo +stable --version` = 1.100.0 drives the build; `rustup which
  --toolchain 1.80.0` provides rustc/rustdoc.
