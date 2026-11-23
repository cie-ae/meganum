# ADR 0004: Tunables storage without external TOML dependency

## Status

Accepted

## Date

2026-11-18

## Context

The specification stores dispatch tunables in
`crates/meganum-core/src/dispatch/tunables.toml`, embedded at compile time
and overridable at runtime. The dependency budget forbids adding a TOML
parsing crate: the runtime dependency set is limited to rayon (behind the
`parallel` feature) and pyo3 (bindings), and a general TOML parser would
violate that for a file the project itself controls. At the same time,
malformed tunables must fail fast and legibly, and runtime overrides coming
from the Python introspection API must pass through the same validation as
file-loaded values.

## Decision

- The tunables file is embedded at compile time via `include_str!`.
- A small purpose-built parser reads exactly the strict subset the file
  needs: tables (`[section]`), `key = value` pairs, integers, booleans,
  quoted strings, and comments. Exotic TOML features (arrays of tables,
  inline tables, multiline strings, dates) are rejected explicitly with a
  typed error rather than approximated.
- Malformed tunables fail fast with a typed error at first use; the engine
  never silently falls back to built-in defaults.
- Runtime overrides flow through the Python introspection API
  (`set_dispatch_overrides`) into the same validated enum space as
  file-loaded values; invalid overrides raise `DispatchError`.
- The tunables file carries a content hash, exposed through
  `meganum.about()`, so diagnostics can identify exactly which calibration
  a running build uses.

## Consequences

- Zero runtime dependencies are added; the dependency budget holds.
- The parser is small, auditable, and tested against both the committed
  tunables file and hostile inputs (truncated files, unknown keys, bad
  types, duplicate tables).
- Future tunable keys require a parser-aware addition rather than a free
  form; this friction is intentional and keeps the calibration surface
  explicit.
- If the tunables format ever outgrows the strict subset, a new ADR must
  weigh adding a dependency against extending the parser.

## Evidence

- Spec: tunables in an embedded TOML file; dependency budget forbidding a
  TOML crate.
- Tests: parser unit tests against the committed file and hostile inputs;
  dispatch tests assert that malformed tunables produce the typed error at
  first use.
- `meganum.about()` exposes the tunables content hash (see
  `docs/dispatcher.md` and the Python reference).
