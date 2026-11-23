# Contributing (Developer Quick Reference)

This page is the short, dev-facing summary; the full process guide lives in
[`CONTRIBUTING.md`](https://github.com/meganum/meganum/blob/main/CONTRIBUTING.md)
at the repository root.

## Build from source

```bash
# Prerequisites: rustup with toolchain >= 1.85 (latest stable; ADR-0008),
# a Python 3.10-3.14 venv
pip install maturin
maturin develop            # builds the extension into the active venv
```

## Run tests

```bash
cargo test                 # Rust engine tests
pytest                     # Python-layer tests (after maturin develop)
```

## Coding standards

- `rustfmt` with default settings; no custom rustfmt config.
- `clippy` must pass with `-D warnings`.
- **Panic discipline.** Library code never panics on bad input; it returns
  typed errors (`MegaNumError`) that map onto the documented Python
  exception hierarchy.
- **unsafe discipline.** `unsafe` is confined to `backend::simd` and the
  limb primitives; every block carries a SAFETY comment and has executing
  tests that exercise it.
- Functions stay around **60 lines**; split earlier if control flow nests.
- **Conventional commits**, one logical change per commit.
- **Never weaken a gate.** CI thresholds, test matrices, and coverage
  requirements can be tightened, not loosened.
- **Tests-first for algorithm code.** Land the `_ref` reference twin and
  its differential tests before or together with the optimized version;
  never optimize untested code.

## ADR triggers

Open an ADR (see [the ADR index](adr/README.md) and the
[template](adr/template.md)) when a change:

- adds a new dependency,
- swaps or replaces an algorithm,
- recalibrates a tunable (the ADR must reference a calibration report),
- changes the public API,
- changes a CI gate,
- adds `unsafe` code.

## Where things live

| Path | Contents |
| --- | --- |
| `crates/meganum-core` | Pure engine: limb primitives, algorithms, dispatcher, backends |
| `crates/meganum-python` | Thin PyO3 adapter |
| `python/meganum/` | Python package (shim re-exporting `meganum._native`), stubs |
| `benches/` | Criterion benchmarks and methodology |
| `docs/adr/` | Architecture decision records |
