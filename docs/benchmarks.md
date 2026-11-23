# Benchmarks

This page describes how MegaNum is measured. It records methodology and
budgets only; **no measured numbers are published on this page**. Results
are recorded per release in committed reports, and methodology details live
in [`benches/METHODOLOGY.md`](https://github.com/meganum/meganum/blob/main/benches/METHODOLOGY.md).

## Methodology

- **Framework.** Criterion benchmarks, driven by reproducible scripts under
  `benches/`.
- **Statistics.** Median of 100+ iterations after a 3-second warmup per
  case; p50 and p95 are reported.
- **Artifacts.** Results are committed as artifacts for every release, so
  each release carries its own evidence.
- **Regression gate.** A change fails review if a gated benchmark regresses
  by more than **+15%** against the target-branch baseline on the same
  runner class.
- **Runner class.** Budget comparisons are only valid within the same
  runner class; never compare numbers across different hardware.

### Reference hardware

The budgets below are defined against this configuration:

- 8 physical cores with AVX2,
- 32 GB RAM,
- release profile with `lto = "thin"` and `codegen-units = 1`.

## Performance budgets

Gated budgets block a release; reported budgets are published for guidance
but do not gate.

| ID | Scenario | Budget | Status |
| --- | --- | --- | --- |
| P1 | import | <= 60 ms | gated |
| P2 | parse 1M digits | <= 200 ms | gated |
| P3 | format 1M digits | <= 200 ms | gated |
| P4 | add 1M + 1M digits | <= 5 ms | gated |
| P5 | mul 1M x 1M digits, parallel | <= 400 ms | gated |
| P6 | mul 1M x 1M digits, forced sequential | <= 1.5 s | gated |
| P7 | divmod 2M / 1M digits | <= 1.2 s | gated |
| P8 | isqrt 1M digits | <= 1.5 s | gated |
| P9 | gcd | <= 2 s | reported |
| P10 | b ** 17 | <= 5 s | reported |
| P11 | modpow, 100k-digit modulus, 64-bit exponent | <= 4 s | reported |
| P12 | MegaFloat.sqrt at 1M digits | <= 3 s | gated |
| P13 | MegaFloat exp / ln at 1M digits | <= 10 s | reported |
| P14 | 4-thread GIL-free scaling | >= 2.5x | gated |

## Reading results honestly

- Budgets are targets validated on the reference configuration; a slower
  machine can miss them without the engine being wrong, and a faster
  machine can pass without proving the budget.
- Per-release committed reports are the only authoritative record of
  measured values. Do not quote numbers from commit messages, issues, or
  documentation pages.
- When adding a benchmark scenario, extend `benches/METHODOLOGY.md` in the
  same change so the methodology stays complete.
