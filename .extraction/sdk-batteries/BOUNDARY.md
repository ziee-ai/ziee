# Chunk sdk-batteries — BOUNDARY

## What is now in the SDK
- **`ziee-build-support`** (new lean crate; re-exported as
  `ziee_framework::build_support`): `worktree_db` (per-worktree keying, moved
  verbatim), `compose_merged_migrations(merged_dir, module_roots)` (G5),
  `ensure_build_db` + one-call `provision_build_db(migrations_dir)` /
  `provision_build_db_named` (G4). Build-DB-free; cheap as a `[build-dependencies]`.
- **`ziee-framework`**: `serve(listener, router)` (connect-info + graceful shutdown,
  G7); `embedded_pg` now defaults the embedded PG dirs from the app data dir + exposes
  `resolve_embedded_paths` (G3), with the `Config::resolve_paths` leak scrubbed; CORS
  permissive log downgraded on loopback.
- **`ziee-auth`**: `DefaultIdentityResolver::new(pool, jwt)` (G1),
  `mount_auth::<R>(router, resolver, jwt, ctx, opts)` + `AuthMountOptions`,
  `NoopAuthEventSink` / `NoopAuthSyncSink`.
- **`ziee-core`**: `ServerConfig::{load_from, discover, load, validate_jwt_secret}` +
  weak-`jwt.secret` refusal (G2).

A second app now mounts working auth in ~5 lines:
```rust
let jwt = Arc::new(JwtService::try_new(cfg.jwt.clone())?);
let resolver = Arc::new(DefaultIdentityResolver::new(pool.clone(), jwt.clone()));
let ctx = AuthContext::new(Arc::new(pool.clone()), None,
    Arc::new(NoopAuthEventSink), Arc::new(NoopAuthSyncSink));
let app = mount_auth(app, resolver, jwt, ctx, AuthMountOptions::default());
ziee_framework::serve(listener, app.finish_api(&mut doc)).await?;
```
and gets `query!` verification via `build_support::provision_build_db(&migrations_dir)`.

## What STAYS app-side (unchanged — additive-only)
- ziee's `ZieeIdentityResolver` + its boot extension-layering in `lib.rs`/`main.rs` +
  `AuthModule::init` (config seed, cleanup loop, trust-flag) — UNCHANGED.
- ziee's monolithic `Config` loader + `resolve_paths` — UNCHANGED (its own loader).
- ziee's `build.rs` DB wipe + multi-source (server+desktop) migrate — kept; only the
  worktree keying + compose + DB-ensure were factored into the SDK calls.

## Boundary checks
- **E5 move-completeness:** `worktree_db` symbols resolve in
  `ziee_build_support::worktree_db`; `compose_merged_migrations`/`ensure_build_db` present. PASS
- **E6 source-deletion:** `src-app/server/build_helper/worktree_db.rs` deleted; the inline
  `compose_merged_migrations` body + provisioning block removed from ziee's build.rs; no
  divergent duplicate remains (only comments reference the old path). PASS
- **E7 transform-declared:** T-1..T-3 declare the build_support move transforms. PASS
- **E8 golden equivalence:** regen ui + desktop → `types.{ui,desktop}.ts` BYTE-IDENTICAL,
  `openapi.{ui,desktop}.json` CANONICALLY-EQUAL vs `.extraction/baseline`. GREEN (see below).
  Generated files `git checkout`-restored after.
- **build_support equivalence:** `migrations-merged` = byte-identical union of the
  module-owned source `*.sql` (`sha256=93b6f632…`, 91==91 files); build DB name
  `ziee_build_<worktree_key>` from the verbatim-moved keying → identical. PASS
- **E9 dual clean-build:** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0,
  `cargo check --workspace` (sdk) = 0. PASS
- **E11 skeleton-agnostic:** `skeleton-server` builds (framework-only, no DATABASE_URL);
  the new `build_support` re-export adds only the lean crate, no domain/auth pull-through. PASS
- **E12 submodule-pin:** the SDK submodule is committed to a build-clean commit; ziee stages
  the updated `sdk` pointer. PASS (staged; not pushed per brief)
- **New-helper tests:** build-support 5/5, embedded_pg 2/2, ziee-core 5/5, ziee-auth
  turnkey 2/2 + noop 2/2 — all pass.

## GOLDEN RESULTS (E8)
- types.ui.ts: BYTE-IDENTICAL
- types.desktop.ts: BYTE-IDENTICAL
- openapi.ui.json: CANONICALLY-EQUAL
- openapi.desktop.json: CANONICALLY-EQUAL

## Residual / follow-ups (NOT this chunk)
- `ziee-auth/build.rs` still carries its own auth-only-DB isolation copy (a duplicate of
  the keying) — could be deduped onto `ziee-build-support` later; out of scope (it's SDK-side,
  not a ziee leak, and its auth-only-DB logic is bespoke).
- G6 (`examples/minimal-app` template) is a separate deliverable, not part of this chunk.
