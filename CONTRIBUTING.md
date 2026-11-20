# Contributing to MegaNum

Thank you for contributing. This guide covers setup, the pull-request
process, test expectations, issue triage, and style. The short developer
reference lives in [`docs/contributing.md`](docs/contributing.md); this
document is the full process.

## Code of Conduct

Everyone participating in the MegaNum project is expected to follow the
[Contributor Covenant Code of
Conduct](CODE_OF_CONDUCT.md). Reports go to
`conduct@meganum.dev`.

## Setup

1. Install Rust through [rustup](https://rustup.rs) (toolchain 1.85 or
   newer; the repository tracks the latest stable release for development
   — see ADR-0008).
2. Create a Python 3.10-3.14 virtual environment and activate it.
3. Install the toolchain and build the extension in-place:

   ```bash
   pip install maturin pytest
   maturin develop
   ```

4. Verify your setup:

   ```bash
   cargo test
   pytest
   ```

If `pytest` cannot import `meganum`, run `maturin develop` again in the
activated venv.

## Pull request process

- Open a PR with the provided template
  (`.github/pull_request_template.md`) and complete its gate checklist.
  The checklist is not optional; reviewers close PRs with unchecked gates
  only when an item genuinely does not apply.
- Write **conventional commits** (`feat:`, `fix:`, `perf:`, `docs:`,
  `refactor:`, `test:`, `chore:`) with **one logical change per commit**.
- Performance-affecting changes must include a **Benchmark impact** section
  with before/after numbers from the same runner class, and the command
  used to obtain them. See [`docs/benchmarks.md`](docs/benchmarks.md) for
  methodology.
- Material deviations from recorded decisions must link an **ADR**
  ([index](docs/adr/README.md)). ADR triggers: new dependency, algorithm
  swap, tunable recalibration (with a calibration report), public API
  change, CI gate change, unsafe addition.
- If your change touches topics tracked in `SPEC_GAPS.md`, list the
  affected entries in the PR and update them in the same change.
- Keep PRs reviewable: prefer a series of small PRs over one large one.

## Test expectations

- **Algorithm changes.** Every optimized algorithm must have a slow,
  obviously-correct reference twin under `algos/_ref` with CI differential
  tests. Land the twin and the differential tests with (or before) the
  optimized code. Property-based tests (`proptest`) accompany algorithm
  work where invariants exist.
- **Correctness claims.** Only claim *correctly rounded* with a proven
  error bound of at most 1/2 ulp and a Ziv-style rounding test; otherwise
  document *rounded per context*.
- **Bit identity.** Any backend change must keep Sequential, Parallel, and
  SIMD outputs bit-identical; differential backend tests must pass.
- **Golden vectors.** Changes that alter outputs in any way (they should
  not) require regenerating golden vectors with their SHA-256 manifest and
  an explicit review note explaining why.
- **unsafe additions.** New `unsafe` blocks require a SAFETY comment and an
  executing test that exercises the safety condition; the PR checklist
  records the review.
- **Hygiene.** No `TODO`/`FIXME`/`unimplemented` markers in `src/`; no
  emoji in code or documentation; `cargo fmt`, `cargo clippy -D warnings`,
  and `cargo doc` must be clean.
- **Never weaken a gate.** CI gates can be tightened, not loosened. If a
  gate is wrong, fix the gate through an ADR, not by deleting the test.

## Benchmark impact

When a change can move performance:

1. Run the affected Criterion suites on the reference configuration
   (8 physical cores AVX2, 32 GB RAM, release profile `lto = "thin"`,
   `codegen-units = 1`).
2. Record p50/p95 before and after in the PR.
3. State whether any gated budget (P1-P14) is affected; the +15% regression
   gate against the target-branch baseline applies.
4. If a tunable needs recalibration, attach the calibration report and open
   the ADR; do not edit thresholds as part of an unrelated PR.

## Issue triage

- **Reproduce first.** Confirm issues against the latest release before
  classifying; note platform, Python version, and wheel vs sdist install.
- **Classify.** Separate correctness bugs, performance regressions, dispatch
  calibration questions, and documentation fixes; correctness bugs take
  priority by design pillar 1.
- **Minimal case.** Reduce to the smallest operand or input that shows the
  problem; MegaNum's deterministic dispatcher makes reproductions stable.
- **Calibration questions.** Questions of the form "why is threshold X set
  to Y" are answered by the calibration ADR; if none exists, the correct
  fix is a calibration report plus ADR, not an ad-hoc threshold change.

## Style

- Rust: `rustfmt` defaults; `clippy -D warnings`; functions around 60
  lines; panic discipline (typed errors, no panics on bad input); unsafe
  confined to `backend::simd` and limb primitives with SAFETY comments.
- Python: typed public surface matching `python/meganum/__init__.pyi`
  (pyright strict-clean); no emoji; no placeholder text in shipped code.
- Docs: concise and technical; tables over prose where clearer; no invented
  benchmark numbers.
- Commits: conventional commit types, imperative subject lines, one logical
  change per commit.
