# Chunk `ziee-test-harness` — the generic integration-test HARNESS (CUT manifest)

Cut the **binary-agnostic core** of ziee's integration-test harness
(`server/tests/common/harness_inner.rs`, 1072 lines) — spawn-the-app-binary,
per-test isolated DB clone-from-template, config write, free-port allocation,
30s health-poll, `Drop` cleanup, and the shared/isolated `data_dir` caching +
per-worktree DB keying — into a new `sdk/crates/ziee-test-harness` crate, leaving
ziee's app-specific parts (the config-YAML content, storage-key/Windows-helper
pre-spawn hooks, the 16-field `TestServerOptions`, and `test_helpers`) behind a
**thin same-file shim** that re-exposes `TestServer` / `TestServerOptions` /
`start*` / `test_helpers` with IDENTICAL names + signatures. The ~272 server +
16 desktop test files (1848 hits) and both `#[path]` include lines compile
UNCHANGED. One injected seam (`HarnessApp`) severs the generic engine from every
app coupling; the compile-time `is_desktop()` switch becomes a runtime `Variant`
the per-crate shim seeds.

## Design-gate — the harness names ONLY the `HarnessApp` seam, never `ziee`

The SDK crate has **zero** `ziee::` references (verified: `git grep -n 'ziee::'
sdk/crates/ziee-test-harness` = 0). The three former couplings resolve to seam
hooks the app implements: `ziee::init_storage_key` + the Windows
`ensure_sandbox_helper_for_tests()` → `HarnessApp::before_spawn`; the
`ziee::file_routing` doc-comment stays in the shim's `data_dir()`. This mirrors
`sdk/desktop/harness::ServerBoot` ("the harness names ONLY the seam").

## Design-gate — `CARGO_MANIFEST_DIR` is a RUNTIME param, never `env!` in the crate

`env!("CARGO_MANIFEST_DIR")` was used at 4 sites (worktree key, repo-root
`.ziee-cache` walk, migration roots, binary walk). Inside a compiled SDK crate it
would resolve to the SDK crate's own dir → split caches / failed template build /
binary-not-found. Every one becomes a `manifest_dir: PathBuf` the CONSUMER passes
from ITS OWN `env!("CARGO_MANIFEST_DIR")` at the shim site (`TestHarness::new`).
`git grep 'CARGO_MANIFEST_DIR' sdk/crates/ziee-test-harness` = 0.

## Design-gate — dev/test crate, build-DB-FREE

`ziee-test-harness` is consumed as a `[dev-dependencies]` path entry (both server
+ desktop). It opens ONLY runtime Postgres connections — no compile-time
`sqlx::query!` macros (all `query!` stay in the shim's `test_helpers`) — so it
needs no build DB to compile, exactly like `ziee-build-support`. Dep versions +
feature sets mirror the server catalog so the single `src-app/Cargo.lock` unifies
without a duplicate build.

## Design-gate — `TestServer` is a same-file shim wrapper (equivalence mechanism)

`TestServer::start()` (no args) must reference ziee's app + the consumer's
`env!("CARGO_MANIFEST_DIR")`, so it CANNOT be an inherent method on a foreign SDK
type (orphan rule). The SDK owns the running-server handle as `SpawnedServer`
(process + tempdirs + `Drop`); the shim owns a thin `TestServer` that wraps it,
copies the three public string fields (`base_url`/`database_name`/`database_url`)
so field access compiles unchanged, and forwards `api_url`/`data_dir`. This is
the same posture as `ziee-file` keeping `routes.rs`/`handlers` in ziee behind the
moved store — the retained same-name surface is what makes the move
equivalence-preserving.

## Files

- move: `src-app/server/tests/common/harness_inner.rs` (generic engine symbols) → `sdk/crates/ziee-test-harness/src/lib.rs`
- new: `sdk/crates/ziee-test-harness/Cargo.toml`
- new: `sdk/crates/ziee-test-harness/src/lib.rs`
- shim: `src-app/server/tests/common/harness_inner.rs` (rewritten in place — ziee's `HarnessApp` impl + `TestServerOptions` + `TestServer` wrapper + `test_helpers` verbatim)
- edit: `src-app/server/Cargo.toml` (dev-dep `ziee-build-support` → `ziee-test-harness`)
- edit: `src-app/desktop/tauri/Cargo.toml` (add dev-dep `ziee-test-harness`)

## Symbols

- symbol: `Variant` (lib.rs) — NEW; replaces compile-time `is_desktop()`
- symbol: `DbConn` (lib.rs) — NEW; parsed connection parts
- symbol: `SpawnFacts` (lib.rs) — NEW; generic per-spawn facts handed to the app
- symbol: `SpawnPlan` (lib.rs) — NEW; app's per-spawn plan (config + argv/env + keep_alive)
- symbol: `HarnessApp` (lib.rs) — NEW; the injected seam trait
- symbol: `TestHarness` (lib.rs) — NEW; generic engine (`new` + `start`)
- symbol: `SpawnedServer` (lib.rs) — moved from `TestServer` struct + `Drop` (the running-server handle)
- symbol: `ensure_test_template` (lib.rs) — moved (generic over `HarnessApp`)
- symbol: `test_template_db` (lib.rs) — moved (now takes app+variant+manifest_dir)
- symbol: `make_isolated_data_dir` (lib.rs) — moved (now takes manifest_dir)
- symbol: `shared_test_app_data_dir` (lib.rs) — moved (now takes manifest_dir)
- symbol: `worktree_suffix` (lib.rs) — moved (now takes manifest_dir)
- symbol: `worktree_db` (lib.rs) — re-exported from `ziee_build_support`
