# Chunk BG — FIX round 1

Blind multi-angle audit (LEDGER.jsonl — 16 findings across 9 angles incl.
`equivalence` + `security`) reconciled against every diff hunk
(AUDIT_COVERAGE.tsv — 28 hunks, ≥3 angles each). All findings resolved DURING
the drift-convergence loop and re-verified, none deferred:

- **lint / unused-import** (`user/handlers/user.rs`): removing the
  `Extension<Arc<EventBus>>` params left `use std::sync::Arc;` unused → `-D
  unused-imports` error. Fixed by deleting the import. Re-verified: `cargo check
  -p ziee` exit 0.

- **lint / unused-import** (`auth/mod.rs`): the `pub use context::{AuthContext,
  AuthEventSink, AuthSyncSink};` re-export was unused (call sites use
  `super::context::…` / `crate::modules::auth::context::…` directly) → error.
  Removed the re-export. Re-verified: exit 0.

- **build / orphan-of-visibility** (`From<JwtConfig> for JwtSettings`): first
  placed in `lib.rs`; the `ziee` BIN re-compiles `modules/` independently and
  does NOT include `lib.rs`, so `main.rs`'s `try_new(config.jwt)` couldn't see the
  impl (E0277). Fixed by MOVING the impl into `core/config.rs` (the shared `core`
  tree compiled by BOTH the lib and the bin). Re-verified: `cargo check -p ziee`
  (lib + bin) exit 0.

- **test / signature follow-through** (`ensure_unique_username`): the `pool`
  param broke two integration-test callers. Updated them to pass the existing
  test pool (TESTS-MOVED). Re-verified: `cargo check --test integration_tests`
  exit 0.

After these, a second blind pass over the full `git diff` hunks (all 28 in
AUDIT_COVERAGE) surfaced no additional equivalence/security/type/boundary
divergence.

**New confirmed findings: 0**
