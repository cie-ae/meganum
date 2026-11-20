# ADR 0001: PyO3 0.30 pin and toolchain policy

## Status

Accepted

## Date

2026-11-18

## Context

The specification requires the latest stable PyO3 minor (0.2x series),
pinned to an exact minor, with an MSRV of 1.80 and abi3-py310 so one wheel
per platform covers Python 3.10-3.14. PyO3 minors routinely bump their own
Rust requirements, so the MSRV and the binding-library pin can collide.
The development toolchain also needs a policy: developers should track
stable Rust for convenience, while CI must enforce the project MSRV
independently. PyO3 0.30 additionally changed its API surface in ways that
affect how binding code is written.

## Decision

- Pin `pyo3 = "0.30"` (any 0.30.x patch release) in
  `crates/meganum-python`. The crate uses the `extension-module` and
  `abi3-py310` features.
- `rust-toolchain.toml` pins `channel = "stable"` for developer
  convenience; CI validates the MSRV of 1.80 in a dedicated matrix job so
  a newer stable compiler cannot hide MSRV breakage.
- Binding code uses the PyO3 0.30 API, specifically:
  - `prepare_freethreaded_python` was removed in 0.30; the interpreter is
    auto-initialized for extension modules.
  - `with_gil` / `allow_threads` were renamed to `attach` / `detach`;
    the adapter uses the new names.

## Consequences

- Bindings are locked to the 0.30.x series until a new ADR changes the
  pin; patch updates within 0.30.x are routine maintenance.
- If a future PyO3 minor raises its Rust requirement above 1.80, the MSRV
  matrix jobs may need feature downgrades (dropping optional features from
  the MSRV job only). If that becomes unsustainable, the MSRV or the PyO3
  pin must be revisited through a new ADR.
- Pinning to an exact minor trades early access to new PyO3 features for a
  stable binding API surface; the Python-facing behavior is unaffected.

## Evidence

- Spec requirements: latest stable PyO3 0.2x pinned to exact minor, MSRV
  1.80, abi3-py310.
- PyO3 0.30 changelog: removal of `prepare_freethreaded_python` for
  extension modules and the `attach`/`detach` renames.
- `rust-toolchain.toml` in the repository root already documents the
  stable pin and points at this ADR.
