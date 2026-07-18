# Chunk C1 — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 17 findings across 12 angles incl.
`equivalence`, `security`, `byte-identity`/`edition-compat`, `boundary`,
`api-surface`, `cross-crate-resolution`, `shim-transparency`, `test-fidelity`,
`build-hygiene`, `canonical-equality`, `desktop-parity`) reconciled against every
diff hunk (`AUDIT_COVERAGE.tsv` — 10 hunks, ≥3 angles each). Two items were
handled DURING implementation (the drift-convergence loop), not deferred:

- **edition-compat** (`ziee-control-mcp/src/catalog.rs`): the first verbatim copy
  failed `cargo check --workspace` with `error: let chains are only allowed in
  Rust 2024 or later` at the two `if let … && …` sites in
  `schema_has_secret_field_rec`. The SDK workspace is edition 2021. Fixed by
  lowering both to the semantically-identical nested `if let` (the exact
  desugaring), keeping the workspace uniformly 2021 rather than special-casing one
  crate to 2024. Re-verified: `cargo check --workspace` exit 0;
  `detects_secret_request_field` (drives those paths) passes.

- **build-hygiene** (`code_sandbox/types.rs`): removing the JSON-RPC type
  definitions left `use serde::{Deserialize, Serialize};` unused (the JSON-RPC
  block was its only consumer), which `-D warnings` would reject. Fixed by
  dropping the import in the same edit. Re-verified: `cargo check -p ziee` +
  `-p ziee-desktop` exit 0 with zero NEW warnings.

All other findings are `verified`: `policy.rs`/`tools.rs` moved byte-for-byte
(diff exit 0); `catalog.rs` differs only by the two lowered let-chains; the
JSON-RPC types + `loopback_host` moved verbatim modulo the
`crate::common::AppError` → `ziee_core::AppError` type-path (identical surface);
the security invariants (loopback pin, secret-body denylist, reachability policy,
mutating-invoke-always-approves) are intact and covered by moved tests; the two
re-export shims keep all 28 downstream call sites + the two boot sites + the
retained handler resolving; the moved core names no app type + no DB (the
`mcp_servers` write stayed app-side); and golden E8 is `types.ts` byte-identical +
`openapi.json` canonically-equal on BOTH ui + desktop.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings: 0**
