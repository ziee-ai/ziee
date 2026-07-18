# Chunk ziee-auth-routes — TESTS-MOVED

## Test files moved: NONE
The moved surface (`handlers.rs`/`routes.rs`/`jwt_extractor.rs`/`session_settings.rs`/
`permissions.rs`) carried NO in-source `#[cfg(test)]` unit tests — they were pure
handler/route/extractor code. So there are no unit tests to relocate into `ziee-auth`.

The auth INTEGRATION tests live in `src-app/server/tests/auth/*` and exercise the
surface over HTTP (real server subprocess) + permission strings — they are NOT
coupled to the moved Rust symbols (verified: no auth test file imports
`handlers::*` / `jwt_extractor::JwtAuth` / a moved permission type; the lone
`AuthProvidersRead` hit is a doc comment, `token_response` hits are a mock method).
They therefore stay app-side unchanged and keep passing against the shimmed paths.

## Test result (touched-module: `auth::`)
The monolithic `integration_tests` binary does NOT compile on this base commit
(4a2391732, MIGRATE-squash) due to **two PRE-EXISTING gaps unrelated to this chunk**
(I touched ZERO test/harness files — `git diff --name-only HEAD -- src-app/server/tests`
is empty):

1. `tests/hub/migration_test.rs` + `tests/code_sandbox/tier2_migrations.rs`
   `include_str!` three numeric migration filenames that MIGRATE-squash DELETED
   (`…036_seed_code_sandbox…`, `…092_rewrite_hub…`, `…131_rewrite_hub…`) → the
   whole binary fails to compile.
2. `tests/common/harness_inner.rs::template_migration_dirs()` still returns the
   flat `server/migrations` dir that MIGRATE-squash DELETED (migrations are now
   module-owned + composed into the build-generated `migrations-merged/`). The
   harness was not updated, so every server-spawning test panics at
   `harness_inner.rs:282` "create migrator … No such file or directory".
   (Plus an environmental note: the harness resolves the `ziee` binary from the
   default `target/debug` paths, ignoring `CARGO_TARGET_DIR` — the known shared-
   target-symlink issue; needs a symlink on this box.)

**Genuine signal obtained** by working those three PRE-EXISTING gaps around
LOCALLY (harness→`migrations-merged`, `include_str!`→`""`, binary symlink) then
REVERTING (tree verified clean, nothing committed):
- `auth::admin_providers_test` → **10 passed / 0 failed** (the moved provider CRUD
  + test-config + 403 permission-gate + sync-emission-to-admins/readers).
- The full server boots successfully with the moved auth routes mounted
  ("ziee backend server started successfully").
- The 7 mock-only auth tests (ldap/oauth connectivity) pass without any workaround.

The full 127-test `auth::` suite was not run to completion here — at
`--test-threads=1` each test spawns a full server (~10-15 s boot) so the suite is
~tens of minutes, and the two harness gaps are MIGRATE-squash's to fix, not this
chunk's. The moved-surface subset + clean boot + byte-identical OpenAPI are the
behavioural proof for this chunk.
