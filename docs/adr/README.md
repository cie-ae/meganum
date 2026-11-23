# Architecture Decision Records

This directory records the material architectural decisions of MegaNum as
short, numbered documents. A decision that is not written down here did not
happen.

## Index

| Number | Title | Status | Date |
| --- | --- | --- | --- |
| [0001](0001-pyo3-pin-and-toolchain.md) | PyO3 0.30 pin and toolchain policy | Accepted | 2026-11-18 |
| [0002](0002-simd-kernel-scope.md) | SIMD kernel scope for v1.0 | Accepted | 2026-11-18 |
| [0003](0003-ntt-design.md) | NTT multiplication design | Accepted | 2026-11-18 |
| [0004](0004-tunables-storage.md) | Tunables storage without external TOML dependency | Accepted | 2026-11-18 |
| [0005](0005-megaint-truediv.md) | MegaInt true division semantics | Accepted | 2026-11-18 |
| [0006](0006-msrv-policy.md) | MSRV policy — core-only rustc 1.80 gate with MSRV-aware resolution | Superseded by 0008 | 2026-11-19 |
| [0007](0007-cargo-deny-v2-schema.md) | cargo-deny 0.21 config migration and tool version pinning | Accepted | 2026-11-19 |
| [0008](0008-toolchain-latest-stable.md) | Toolchain tracks latest stable; rustc-1.80 MSRV gate removed | Accepted | 2026-11-19 |
| [0009](0009-extension-module-feature-gating.md) | pyo3 extension-module is an opt-in, wheel-builds-only feature | Accepted | 2026-11-19 |

Start new ADRs from the [template](template.md) and name the file
`NNNN-short-title.md` with the next free number.

## How to write an ADR

- **One decision per ADR.** If a discussion produces two decisions, write
  two ADRs.
- **Sections** (see the template): *Status* (proposed / accepted /
  superseded by NNNN), *Date*, *Context* (the forces and constraints at
  play), *Decision* (what we do, stated actively), *Consequences* (what
  becomes easier, harder, or impossible), and *Evidence* (the benchmark or
  calibration reports, spec sections, and tests that justify it).
- **Evidence over opinion.** Claims about performance or calibration must
  cite a committed benchmark or calibration report; claims about
  correctness must cite tests or proofs.
- **Accepted ADRs are immutable history.** To change a decision, write a
  new ADR and mark the old one superseded; never rewrite an accepted ADR
  in place.
- **Link ADRs from code review.** The PR gate checklist requires an ADR
  link for material deviations; see
  [CONTRIBUTING.md](../../CONTRIBUTING.md).
