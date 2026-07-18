# Chunk sdk-batteries — TESTS-MOVED

## Moved (with the moved code)
- **T-worktree-suffix-stable** [ported→sdk] file:
  `sdk/crates/ziee-build-support/src/worktree_db.rs` covers: `worktree_key` (stable, 8-hex).
- **T-worktree-server-desktop-share-key** [ported→sdk] file:
  `sdk/crates/ziee-build-support/src/worktree_db.rs` covers: `worktree_key` (one key/worktree).
- **T-worktree-different-differ** [ported→sdk] file:
  `sdk/crates/ziee-build-support/src/worktree_db.rs` covers: `worktree_key`.
- **T-sentinel-detection** [ported→sdk] file:
  `sdk/crates/ziee-build-support/src/worktree_db.rs` covers: `should_auto_isolate`.
- **T-with-database-swaps-path** [ported→sdk] file:
  `sdk/crates/ziee-build-support/src/worktree_db.rs` covers: `with_database`.

These 5 were the ONLY tests attached to the moved code (`build_helper/worktree_db.rs`);
they moved VERBATIM into the SDK crate. `compose_merged_migrations` +
`ensure_build_db` were inline `build.rs` code with no unit tests (build scripts aren't
unit-tested); their equivalence is proven by the fingerprint check (DRIFT-1) + the
golden regen (BOUNDARY E8), not by a moved test.

## New (for the additive helpers — a smoke/unit test each)
- **T-resolve-embedded-paths-fills** [new→sdk] `ziee-framework/src/embedded_pg.rs` covers:
  `resolve_embedded_paths` (unset dirs → `<data dir>/postgres[-data]`, no `Config::resolve_paths` ref).
- **T-resolve-embedded-paths-preserves** [new→sdk] same file covers: override-wins (equivalence).
- **T-config-loads-valid / -refuses-placeholder / -refuses-short / -ignores-extra-keys /
  -discover-env** [new→sdk] `ziee-core/src/config.rs` covers: `ServerConfig::{load_from,
  discover,validate_jwt_secret}`.
- **T-default-resolver-implements-identity-resolver** [new→sdk] `ziee-auth/src/auth/turnkey.rs`
  covers: `DefaultIdentityResolver: IdentityResolver` (compile-time trait proof).
- **T-mount-auth-wires-full-surface** [new→sdk] same file covers: `mount_auth` (finish_api →
  axum::Router; spec populated) over a lazy pool.
- **T-noop-sinks-send-sync-inert / -fit-authcontext-slots** [new→sdk]
  `ziee-auth/src/auth/context.rs` covers: `NoopAuthEventSink` / `NoopAuthSyncSink`.

## Stays app-side (unchanged)
- `src-app/server/tests/common/harness_inner.rs` — the harness itself (not a test) now
  `use`s `ziee_build_support::worktree_db` (dev-dep) instead of the deleted `#[path]`
  include; its per-worktree suffix derivation is byte-identical, so every server-spawning
  integration test keeps its unchanged test-template DB name. No integration test file
  was edited.

## Test result (touched scope)
Unit tests for every new/moved symbol pass:
- `ziee-build-support`: 5/5 (worktree_db).
- `ziee-framework`: embedded_pg `resolve_*` 2/2.
- `ziee-core`: `load_from_tests` 5/5.
- `ziee-auth`: `turnkey::` 2/2 + `noop_sink_tests` 2/2.
The full ziee integration suite is NOT this chunk's gate (the pre-existing MIGRATE-squash
harness debt noted in prior chunks still applies); the equivalence proof (fingerprint +
golden) + the clean dual-crate builds are the behavioural proof here.
