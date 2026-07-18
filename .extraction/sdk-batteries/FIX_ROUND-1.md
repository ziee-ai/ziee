# Chunk sdk-batteries — FIX_ROUND-1 (blind multi-angle audit → fix)

Blind audit ran the angles in `LEDGER.jsonl` over the full diff (SDK submodule
`git diff --cached` + ziee `git diff --cached -- src-app`), with whole-diff hunk
coverage in `AUDIT_COVERAGE.tsv` (≥3 angles/hunk).

## Candidate findings triaged
- **[considered] provision_build_db nested-runtime panic** — could ziee call it and
  double-fault ("runtime within a runtime")? Verdict: NOT a defect. ziee's build.rs is
  `#[tokio::main]` and uses the async `ensure_build_db`; only a plain-`fn main()` app
  uses `provision_build_db` (which owns a current-thread runtime). Documented on both.
- **[considered] mount_auth extension-layer ordering** — do protected routes added
  after `mount_auth` miss the resolver extension? Verdict: real semantics, documented
  as the contract (call after own routes / serve the returned router) — matches ziee's
  global-layer boot; the smoke test proves the wired path finishes into an axum Router.
- **[considered] embedded_pg default fires for ziee** — would a fresh default path
  change ziee's cluster location? Verdict: no. ziee sets both dirs via resolve_paths, so
  its `Some(..)` wins; the `unwrap_or_else` default is dead for ziee (byte-identical).
- **[considered] CORS downgrade hides a real prod misconfig** — Verdict: no. The
  downgrade is gated on a LOOPBACK `server.host`; a public bind still emits the loud
  `SECURITY:` ERROR. Only the log level changes; the CORS layer itself is untouched.
- **[considered] weak-secret denylist drift vs ziee-auth** — Verdict: kept in lockstep
  (same 6 entries incl. the dev.example placeholder + the 32-byte minimum). A comment
  flags the lockstep requirement; both refuse the same set.
- **[considered] DefaultIdentityResolver weaker than ZieeIdentityResolver** — Verdict:
  no. Same order + same status codes (401 missing/invalid/not-found, 403 inactive), with
  inactive rejected before group load. Pool-bound vs global-Repos-bound is the only diff.
- **[considered] build-dep on ziee-build-support double-compiles the framework** —
  Verdict: no. build.rs depends on the LEAN crate directly (sqlx runtime + tokio), NOT
  ziee-framework, so the aide/axum/postgresql_embedded graph is never pulled as a build-dep.

## Result
New confirmed findings: 0
