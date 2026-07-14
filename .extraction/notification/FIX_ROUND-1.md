# Chunk `notification` — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 10 findings across 9 angles incl.
`equivalence`, `migration-integrity`, `canonical-equality`, `shim-transparency`,
`cross-crate-resolution`, `boundary`, `test-fidelity`, `desktop-parity`)
reconciled against every diff hunk (`AUDIT_COVERAGE.tsv` — 7 hunks, ≥3 angles
each).

The scope boundary was decided UP FRONT (not a deferred fix): `repository.rs`
(compile-time `query_as!` whose fkeys reference other modules' tables → can only
verify against the app's FULL merged build DB, unlike self-contained `ziee-auth`)
and `events.rs` (names the concrete `SyncEntity::Notification`) stay in ziee. So
`ziee-notification` is a build-DB-free crate that CARRIES the schema
(`migrations/`) but not the queries — the honest equivalence-preserving boundary.

All findings are `verified`: `models.rs` moved byte-for-byte (diff exit 0);
`permissions.rs` differs only by the one trait-import line; both migrations moved
byte-for-byte (checksums/versions preserved, globbed once) and the N7 composition
is proven by ziee's build.rs provisioning the merged build DB + the 10 `query_as!`
verifying (`cargo check -p ziee` exit 0); the shim keeps every call site +
`scheduler/dispatch.rs` resolving; the crate names no app type + no `query!` + no
build.rs; and golden E8 is `types.ts` byte-identical + `openapi.json`
canonically-equal on BOTH ui + desktop.

Re-audit of the diff surfaced no new issues.

**New confirmed findings: 0**
