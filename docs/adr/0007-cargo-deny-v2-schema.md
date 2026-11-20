# ADR 0007: cargo-deny 0.21 config migration and tool version pinning

## Status

Accepted

## Date

2026-11-19

## Context

The first full CI run failed the `supply-chain` job: `cargo-deny` was
installed **unpinned** (latest at run time), and cargo-deny >= 0.19 removed
the classic per-class severity fields. The committed `deny.toml` still used
the classic schema (`vulnerability = "deny"`, `unmaintained = "warn"`,
`yanked = "warn"`), so the check failed during config deserialization with
`error[unexpected-value]: expected '["all", "workspace", "transitive",
"none"]'` — the gate had never actually run.

The spec's supply-chain policy (deny.toml comments, spec sections on
dependency hygiene) is:

* vulnerabilities: hard error;
* unmaintained crates: surfaced, **not** blocking;
* yanked releases: surfaced, **not** blocking;
* bignum crates (`num-bigint`, `rug`, `gmp-mpfr-sys`): banned.

The new schema has no "warn" level for unmaintained advisories; it only
offers error scopes (`all` / `workspace` / `transitive` / `none`).
Meanwhile `vulnerability = "deny"` is gone because **every** vulnerability
advisory is now unconditionally an error.

## Decision

1. **Pin the tool to the config**: `taiki-e/install-action` now installs
   `cargo-deny@0.21.0` (latest stable, 2026-11-16) in `ci.yml` and
   `nightly.yml`. `deny.toml` targets exactly that schema. Bumping the pin
   is a paired change with `deny.toml`, never a silent drift.
2. **Policy mapping in `deny.toml`** (no gate weakening — the config never
   parsed before, so this is the first working encoding of the spec policy):
   * vulnerabilities: always errors in cargo-deny >= 0.19 (strictly
     stronger than the old `vulnerability = "deny"`); per-advisory
     exceptions require a documented `ignore` entry.
   * unmaintained: `unmaintained = "none"` in cargo-deny (non-blocking,
     per spec) — the *surfacing* half of the policy is provided by the
     `cargo audit` step that runs in the same job, which warns on
     unmaintained crates without failing.
   * yanked: `yanked = "warn"` (unchanged behavior, still valid schema).
   * licenses / bans / sources sections: unchanged (still valid in 0.21).
3. `cargo-audit` itself stays unpinned: it reads no repository-owned config
   and its advisory database is fetched fresh each run, so its findings are
   always against the current RustSec DB.

## Consequences

- `cargo deny check advisories bans licenses sources` passes and exits 0 on
  the current dependency tree (verified against cargo-deny 0.21.0 locally).
- Unmaintained crates no longer produce cargo-deny *warnings*; they appear
  in the job log as `cargo audit` warnings instead. Both halves of the
  spec policy (surface + not blocking) are preserved.
- The pin adds a manual maintenance step: when a cargo-deny release is
  wanted, bump `tool: cargo-deny@<version>` and re-validate `deny.toml`
  in the same commit. Supply-chain gate changes can no longer arrive
  unannounced from upstream.
- cargo-deny 0.21 emits `license-not-encountered` warnings for allow-list
  entries not present in the current tree (forward-looking policy entries);
  these are informational and do not fail the check.

## Evidence

- Local run (2026-11-19, cargo-deny 0.21.0 x86_64-musl binary, sha256
  `a2170b85…5a903` verified against the release checksum):
  `advisories ok, bans ok, licenses ok, sources ok`.
- Failing CI log (2026-11-18): cargo-deny deserialization error at
  `deny.toml:13:17` on `unmaintained = "warn"`.
- cargo-deny advisories config reference
  <https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html>
  (schema consulted 2026-11-19).
