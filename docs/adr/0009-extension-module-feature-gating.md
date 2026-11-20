# ADR 0009: pyo3 extension-module is an opt-in, wheel-builds-only feature

## Status

Accepted

## Date

2026-11-19

## Context

The macOS CI leg failed at link time while compiling `meganum-python`:

```
error: linking with `cc` failed: exit status: 1
  = note: Undefined symbols for architecture arm64:
            "_PyBaseObject_Type", referenced from: ...
          ld: symbol(s) not found for architecture arm64
error: could not compile `meganum-python` (lib) due to 1 previous error
```

The link line contained `-Wl,-exported_symbols_list`, `-nodefaultlibs`
and no libpython — the signature of a pyo3 build with the
`extension-module` feature enabled. `crates/meganum-python/Cargo.toml`
did indeed enable it unconditionally:

```toml
pyo3 = { workspace = true, features = ["abi3-py310", "extension-module"] }
```

With `extension-module`, pyo3 deliberately does **not** link libpython:
a Python extension (.so/.dylib/.pyd) has its Python C API symbols
resolved by the interpreter at import time. That is correct for wheel
builds and wrong for everything else — any binary cargo links (test
harnesses, unittests, benches, examples) is not loaded by an
interpreter, so its `_Py*` references stay undefined and the link fails
(macOS errors on undefined symbols in this configuration; Linux
executables fail the same way at a different stage).

## Decision

1. `pyo3/extension-module` is **removed from the dependency features**
   of `meganum-python`. The crate declares its own opt-in feature:

   ```toml
   [features]
   extension-module = ["pyo3/extension-module"]
   ```

   with `default = []`, so plain `cargo build/test/clippy` never links
   a libpython-less binary.

2. `pyproject.toml` enables it **only for wheel builds**:
   `features = ["parallel", "simd", "extension-module"]` under
   `[tool.maturin]`. Wheel builds resolve the Python C API at import
   time, which is the intended production configuration (abi3-py310,
   one wheel per platform for CPython 3.10-3.14).

3. CI never links `meganum-python` outside maturin:
   - clippy runs `cargo clippy` (check-only, no linking) on all three
     OSes;
   - `cargo test` is scoped to the pure-Rust `meganum-core`;
   - binding behaviour is verified by the pytest differential suite
     against the installed wheel on all three OSes
     (`tests/integration`, `tests/differential`, `scripts/smoke_api.py`).

## Consequences

- The macOS (and any libpython-less) link failure is structurally
  impossible: cargo-level builds always link libpython, wheel builds
  always resolve at import time.
- `cargo test -p meganum-python` would now work only where a Python
  development library is linkable; it is intentionally not part of CI
  (runner images do not guarantee libpython dev files). The pytest
  differential suite is the binding gate.
- Anyone hand-running `cargo test --features extension-module -p
  meganum-python` reintroduces the broken configuration; the feature
  docs and this ADR record that as unsupported.
- No runtime behaviour change: the shipped wheel is byte-for-byte the
  same configuration as before (extension-module + abi3-py310 +
  parallel + simd).

## Evidence

- Failing CI log (2026-11-19): macOS arm64 undefined `_Py*` symbols
  while linking `meganum-python (lib)` — the exact configuration this
  ADR forbids outside maturin.
- pyo3's documented behavior: `extension-module` skips the libpython
  link because the interpreter resolves the symbols.
- Local verification (2026-11-19, Linux x86_64, rustc 1.100.0):
  `cargo check -p meganum-python` (without the feature) succeeds;
  `maturin build` (with the feature) produces a wheel that imports and
  passes `pytest tests` and `scripts/smoke_api.py`.
