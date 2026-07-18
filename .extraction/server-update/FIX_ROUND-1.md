# Chunk `server-update` — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 9 findings across 9 angles incl.
`equivalence`, `canonical-equality`, `api-surface`, `shim-transparency`,
`cross-crate-resolution`, `boundary`, `test-fidelity`, `desktop-parity`)
reconciled against every diff hunk (`AUDIT_COVERAGE.tsv` — 5 hunks, ≥3 angles
each).

The scope-boundary decision was made UP FRONT (not a deferred fix): `checker.rs`
is retained because moving it would resolve `env!("CARGO_PKG_VERSION")` to the
crate's `0.0.0` (an observable `/api/server-update/status` regression) and its test
names `crate::core::config::UpdateCheckConfig`. Only the two files with zero
app/version coupling (`types.rs`, `permissions.rs`) were moved — the honest
DB-free-decoupled boundary.

All findings are `verified`: `types.rs` moved byte-for-byte (diff exit 0);
`permissions.rs` differs only by the two trait-import lines (same
`ziee-identity` traits ziee re-exports); the `server_update::read` string + the
403 example are unchanged; the moved 1 test passes; the crate names no app type +
no DB + no `env!`; and golden E8 is `types.ts` byte-identical + `openapi.json`
canonically-equal on BOTH ui + desktop.

Re-audit of the diff surfaced no new issues.

**New confirmed findings: 0**
