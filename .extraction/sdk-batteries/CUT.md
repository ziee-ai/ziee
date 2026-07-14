# Chunk sdk-batteries — CUT manifest

Ship the "batteries-included" ergonomic surface the CytoAnalyst dogfooding
(`cytoanalyst/SDK_GAPS.md`, G1–G7) proved missing: helpers that turn ~300 lines
of copied-from-ziee boot glue into a few SDK calls. **Mixed shape:** P0-a
(`build_support`) is an equivalence-preserving **MOVE** (E5/E6/E7 apply); P0-b is
an in-SDK **fix** of an existing crate; P0-c/P0-d and P1 are **ADDITIVE** new
helpers (no ziee behaviour change; E5/E6/E7 do not apply to them).

## Files

### Moved (app → SDK), equivalence-preserving — P0-a (build_support, decision #11 / N7 / G4 / G5)
- move: `src-app/server/build_helper/worktree_db.rs` (163 L) →
  `sdk/crates/ziee-build-support/src/worktree_db.rs` (164 L). VERBATIM — the FNV-1a
  per-worktree keying (`worktree_key`/`should_auto_isolate`/`with_database` +
  `DEFAULT_BUILD_DB_URL`/`LOCAL_BUILD_CLUSTER` consts) + its 5 unit tests, moved
  1:1 (only the module doc-comment gained a sdk-batteries provenance line). The
  app-side file is DELETED.
- move: `src-app/server/build.rs::compose_merged_migrations()` (the ~60-line body)
  → `sdk/crates/ziee-build-support/src/migrations.rs::compose_merged_migrations(merged_dir, module_roots)`.
  Same glob → sort → basename-collision-guard → copy → `cargo:rerun-if-changed`
  logic; parameterized on the merged-dir + module-roots so ziee supplies its own
  and the composed output is BYTE-IDENTICAL. ziee's `build.rs` keeps a 6-line
  wrapper that passes its two roots.
- move: the inline per-worktree DB-provisioning block in `build.rs::main()`
  (admin-connect → CREATE-if-missing → fallback) →
  `sdk/crates/ziee-build-support/src/build_db.rs::ensure_build_db(base, db_name)`.
  Behaviour identical (same `ziee_build_<key>` name, same swallowed
  duplicate-database race, same fall-back-to-base on cluster-unreachable).

### New (SDK)
- new: `sdk/crates/ziee-build-support/` — the lean, build-DB-free crate (sqlx
  runtime + tokio, NO `query!` macros) home of the moved build glue, PLUS the
  ergonomic one-call `provision_build_db(migrations_dir)` / `provision_build_db_named`
  (G4: derive isolated URL → create → wipe → migrate → return URL, own runtime).
- new: `sdk/crates/ziee-auth/src/auth/turnkey.rs` — `DefaultIdentityResolver`
  (P0-c / G1), `mount_auth::<R>` + `AuthMountOptions` (one-call auth).

### Edited (SDK) — additive / fix
- edit: `ziee-framework/src/lib.rs` — `pub use ziee_build_support as build_support;`
  (the canonical `ziee_framework::build_support::…` path the brief names) +
  `pub use app_builder::serve;` (P1).
- edit: `ziee-framework/Cargo.toml` — dep `ziee-build-support`; tokio `+signal,+net`.
- edit: `ziee-framework/src/embedded_pg.rs` — **P0-b**: scrub the ziee-only
  `Config::resolve_paths` panic-string leak; default `installation_dir`/`data_dir`
  to `<app data dir>/postgres[-data]` when unset (`default_embedded_dir` +
  public `resolve_embedded_paths`). No-op for ziee (it always fills them).
- edit: `ziee-framework/src/app_builder.rs` — **P1**: `serve(listener, router)`
  (connect-info + graceful shutdown, fixes G7) + CORS permissive-`SECURITY:`-ERROR
  downgraded to `debug` on a loopback bind.
- edit: `ziee-auth/src/auth/context.rs` — **P0-c**: `NoopAuthEventSink` /
  `NoopAuthSyncSink` (G1) — the two trait impls every app hand-wrote.
- edit: `ziee-auth/src/auth/mod.rs` — export the turnkey + noop symbols (routes-gated
  for turnkey; noop sinks always-on).
- edit: `ziee-core/src/config.rs` — **P0-d / G2**: `ServerConfig::{load_from,
  discover, load, validate_jwt_secret}` + weak-`jwt.secret` refusal (min 32 bytes
  + placeholder denylist, incl. the `dev.example.yaml` value).
- edit: `ziee-core/Cargo.toml` — promote `serde_norway` dev→normal dep; `tempfile` dev-dep.
- edit: `sdk/Cargo.lock` — the new crate.

### App-side → THIN CONSUMER (P0-a wiring only; equivalence-preserving)
- edit: `src-app/server/build.rs` — drop `#[path] mod worktree_db` + the inline
  compose fn body + the inline provisioning block; `use ziee_build_support::worktree_db`
  (build-dep); call `ensure_build_db` + `compose_merged_migrations`.
- edit: `src-app/desktop/tauri/build.rs` — `#[path]` include → `use
  ziee_build_support::worktree_db` (build-dep).
- edit: `src-app/server/tests/common/harness_inner.rs` — `#[path]` include → `use
  ziee_build_support::worktree_db` (dev-dep).
- edit: `src-app/server/Cargo.toml` (+build-dep +dev-dep), `desktop/tauri/Cargo.toml`
  (+build-dep), `src-app/Cargo.lock`.
- delete: `src-app/server/build_helper/worktree_db.rs`.

## Symbols
- moved: `worktree_key`, `should_auto_isolate`, `with_database`,
  `DEFAULT_BUILD_DB_URL`, `LOCAL_BUILD_CLUSTER`, `stable_suffix`, `worktree_root`
  (→ `ziee_build_support::worktree_db`); `compose_merged_migrations` (→ `…::migrations`).
- new SDK: `ensure_build_db`, `provision_build_db`, `provision_build_db_named`,
  `DefaultIdentityResolver`, `mount_auth`, `AuthMountOptions`, `NoopAuthEventSink`,
  `NoopAuthSyncSink`, `resolve_embedded_paths`, `default_embedded_dir`,
  `ServerConfig::{load_from,discover,load,validate_jwt_secret}`, `MIN_JWT_SECRET_LEN`,
  `ziee_framework::serve`.

## Design-gate (resolutions in TRANSFORMS.md)
1. **build_support home** — decision #11 says "SDK build-support **crate**", the
   brief names `ziee_framework::build_support::…`. Resolution: a lean crate
   `ziee-build-support` (the crate, shared by both apps' `build.rs`) **re-exported**
   by ziee-framework as `build_support` (the canonical path). Both true; build.rs
   stays lean (doesn't drag the whole framework as a build-dep).
2. **build_support equivalence** — the compose + keying logic moved VERBATIM,
   parameterized; ziee's `migrations-merged` composes byte-identically + the build
   DB provisions exactly as before (proven — see TRANSFORMS §Equivalence).
3. **embedded_pg no-op for ziee** — ziee already fills installation_dir/data_dir,
   so its `Some(...)` values win over the new defaults → byte-identical behaviour.
4. **N2 additive-only OpenAPI** — the new helpers add NO aide operations / schemas;
   ziee mounts auth with the unchanged `ZieeIdentityResolver`. Golden byte-identical
   on both surfaces (see BOUNDARY E8).
