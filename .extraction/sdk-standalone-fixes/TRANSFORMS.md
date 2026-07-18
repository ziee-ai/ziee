# Chunk `sdk-standalone-fixes` — TRANSFORMS

Every edit, and why it is behavior-/equivalence-preserving for ziee.

## N-1 — notification FK relocation

- **T1 (del + recreate, byte-identical):**
  `sdk/crates/ziee-notification/migrations/202607144180_notification_fkeys.sql`
  is `git rm`'d from the SDK submodule and recreated at
  `server/src/modules/notification/migrations/202607144180_notification_fkeys.sql`
  with IDENTICAL bytes (sha256 `02abd32cb2999dbc89135486f98f920aaceae89db8663566e905792a3418b709`
  before and after). No content change → the 4 FK constraints (`conversation_id`
  → conversations, `scheduled_task_id` → scheduled_tasks, `user_id` → users,
  `workflow_run_id` → workflow_runs) are re-authored verbatim, ziee-side.
- **Merge composition is location-agnostic:** the merged dir is a multiset of
  (basename → bytes). The file previously arrived via the `sdk/crates` source
  root; it now arrives via the `src/modules` source root. Same basename, same
  bytes, appears exactly once → merged set unchanged. Verified: reconstructed the
  pre-change merged set and `diff -rq` vs the current `migrations-merged` is EMPTY
  (both 92 files).
- **`ziee-notification/Cargo.toml`:** doc-comment only (the crate's `migrations/`
  are now domain-agnostic + standalone-applicable). No dependency / code change.

## N-2 — `compose_merged_migrations_from`

- **T2 (additive fn):** `compose_merged_migrations_from(merged, app_module_roots,
  sdk_crate_migration_dirs)` factors out the body; `app_module_roots` are globbed
  for `*/migrations` (unchanged behavior), then each explicit
  `sdk_crate_migration_dirs` entry is added directly iff it `is_dir()`. The rest
  (collision-guard, version-sort, `cargo:rerun-if-changed`) is shared verbatim.
- **T3 (delegation):** `compose_merged_migrations(merged, roots)` now calls
  `compose_merged_migrations_from(merged, roots, &[])` — same output as before for
  any existing caller (e.g. the skeleton-server / examples).
- **T4 (ziee build.rs):** ziee passes `app_module_roots=[src/modules]` +
  `sdk_crate_migration_dirs=[ziee-auth, ziee-file, ziee-notification]/migrations`.
  These three are EXACTLY the SDK crates that ship a `migrations/` dir, i.e. the
  set the old `glob(sdk/crates)` produced. Merged set byte-identical (verified).

## N-3 — `AuthModule`

- **T5 (new module, feature-gated):** `sdk/crates/ziee-auth/src/auth/module.rs`
  defines `AuthModule` + a `#[distributed_slice(MODULE_ENTRIES)]` static, gated
  `#[cfg(feature = "module")]`. `AuthModule::init` builds `JwtService` (from
  `ServerConfig.jwt`, weak-secret-refusing), `DefaultIdentityResolver`, and a
  no-op-sink `AuthContext`; sets the trust flag; spawns the one-time
  session-settings seed. `register_routes` mounts `auth_routes` /
  `auth_admin_routes` (→ `/api/auth/*` after `build_api_router` nests) and layers
  the resolver/JWT/AuthContext extensions on the combined router.
- **T6 (mod wiring):** `auth/mod.rs` gains `#[cfg(feature="module")] pub mod
  module;` + `pub use module::AuthModule;`.
- **T7 (Cargo):** new `module = ["routes", "dep:linkme"]` feature (default-OFF),
  optional `linkme` dep, and `tower`/`http-body-util` dev-deps for the oneshot
  test. NONE of this is reachable from ziee (ziee depends on `ziee-auth` with the
  default `["routes"]` only), so ziee's compiled artifact + OpenAPI are unchanged.

## Equivalence for ziee (why golden + fingerprint are unchanged)

- N-1/N-2 touch only migration composition + file location → identical merged
  migration set → identical final schema (EA fingerprint unchanged) → and OpenAPI
  is route-/type-derived, not schema-derived, so it is untouched.
- N-3 adds only code behind a feature ziee never enables → ziee links no new
  symbol → route set + schemars types unchanged → OpenAPI byte-identical.
