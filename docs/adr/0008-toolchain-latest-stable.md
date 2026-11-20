# ADR 0008: toolchain tracks latest stable; rustc-1.80 MSRV gate removed

## Status

Accepted (maintainer directive, 2026-11-19)

## Date

2026-11-19

## Context

ADR-0006 introduced a core-only rustc-1.80 MSRV gate with MSRV-aware
dependency resolution after the full-workspace 1.80 matrix proved
unachievable (PyO3 0.30 declares MSRV 1.85; the dev-dependency ecosystem
moved past 1.80). The attached CI log shows that gate failing in practice
anyway: on the runners, the fallback resolution did not constrain the
dependency graph, and cargo refused to build with
`criterion@0.8.2 requires rustc 1.86` under rustc 1.80 — a job that could
never pass without further machinery.

The maintainers have now directed that MegaNum use the **latest stable
Rust** instead of 1.80. The current latest stable is 1.100.0 (released
2026-11-12). Verified floors in the dependency tree: PyO3 0.30.1 declares
`rust-version = "1.85"`; proptest 1.12.0 declares `rust-version = "1.88"`;
the committed lock pins criterion 0.5.1 / proptest 1.12.0.

## Decision

- `rust-toolchain.toml` keeps `channel = "stable"`, which now means
  "latest stable" (1.100.0 at the time of writing); it tracks new stable
  releases as they ship.
- The dedicated rustc-1.80 MSRV CI job is **removed**. CI compiles and
  tests with the latest stable toolchain on all legs.
- The workspace `rust-version` floor is raised from 1.80 to **1.85**
  (the PyO3 0.30 requirement); `clippy.toml` `msrv` follows. The dev
  tree needs 1.88 (proptest 1.12) — documented here and in the
  dev-dependency comment; contributors on older toolchains get cargo's
  precise "requires rustc X" error rather than a silently broken gate.
- Dev-dependency version floors stay broad (`proptest = "1"`,
  `criterion = "0.5"`); the committed lock continues to pin the exact
  versions CI uses.
- `simd-avx512` remains an opt-in feature (ADR-0002 unchanged): its
  intrinsics stabilized in Rust 1.89, so keeping it opt-in preserves the
  default build for slightly older toolchains, but it no longer exists
  for MSRV compliance.

This decision supersedes the MSRV machinery of ADR-0006 (which is marked
superseded) and the MSRV-validation sentence of ADR-0001; the PyO3 0.30
pin itself is unchanged.

## Consequences

- CI can never again fail on a "toolchain older than the dependency
  tree" class of error; the whole workspace builds on one current
  compiler across all three OSes.
- The project MSRV promise to downstream Rust consumers is now
  "1.85, realistically latest stable", instead of a tested 1.80 gate.
  This is accepted trade-off per the maintainer directive; if a tested
  MSRV is wanted again later, it should be re-introduced by a new ADR
  with a resolution strategy verified on real runners.
- `rust-version` values in the dependency tree are enforced by cargo
  itself; a future dependency bump that raises the floor shows up as a
  precise build error on older local toolchains, and CI (latest stable)
  keeps passing.

## Evidence

- CI log (2026-11-19): `error: rustc 1.80.0 is not supported by the
  following package: criterion@0.8.2 requires rustc 1.86` (two runner
  legs), under the previous MSRV-aware job.
- Registry metadata extracted from the pinned sources: pyo3-0.30.1
  `rust-version = "1.85"`; proptest-1.12.0 `rust-version = "1.88"`.
- Local verification (2026-11-19, rustc 1.100.0): full workspace check,
  meganum-core test suite (default and `parallel,simd`), clippy
  `-D warnings`, criterion bench with `--save-baseline`, maturin wheel
  build and the pytest differential suite all pass.
