# Chunk `sdk-surfaces` — two mountable backend surfaces (CUT manifest)

Two small, clean extractions so a second app gets realtime sync + first-run
onboarding "for free":

1. **`sync_routes()`** — finish ziee-sync: move the last app-agnostic half of the
   sync module (the `GET /api/sync/subscribe` SSE handler) into
   `ziee_framework::sync` as a mountable `sync_routes::<R, S>()`, generic over the
   app's identity resolver + a `SyncSurface`. The concrete `SyncEntity` enum (58
   domain variants) + wire types STAY in ziee.
2. **`ziee-onboarding`** — move the generic guide/step completion tracker into a
   new schema-bound SDK crate (mirror `ziee-notification`, but the repository +
   its own build DB move too, since its queries are self-contained).

SDK submodule base sha `be734bd` (branch `feat/sdk-surfaces`).

---

## Deliverable 1 — `sync_routes()` (CUT with move)

### Files move: INTO `ziee-framework` (submodule `sdk/`)

- new `crates/ziee-framework/src/sync/routes.rs` — the moved
  `subscribe_sync` handler (the `tokio::select!` over {channel recv, 60s
  re-check, JWT-`exp` deadline}) + `subscribe_sync_docs` + the mountable
  `sync_routes::<R, S>()`. Generic over `R: IdentityResolver` (the pluggable auth
  mechanism) + `S: SyncSurface` (the new app-surface seam). Adds `RecheckOutcome`
  + the `AccessTokenExp<R>` deadline extractor. `recheck_interval()` (incl. the
  debug-only `SYNC_RECHECK_TICK_MS` seam) moved verbatim.
- edit `crates/ziee-framework/src/sync/mod.rs` — `pub mod routes;` + re-export
  `{RecheckOutcome, SyncSurface, sync_routes}`.
- edit `crates/ziee-framework/src/lib.rs` — re-export the three at crate root.
- edit `crates/ziee-framework/src/permissions/resolver.rs` — add
  `IdentityResolver::access_token_exp(&self, &Parts) -> Option<i64>`
  (default `None`); the resolver owns token verification, so it surfaces the
  access token's `exp` to bound the SSE stream without the framework naming a
  concrete JWT scheme.
- edit `crates/ziee-framework/Cargo.toml` — add `async-stream` + `futures-util`
  + `chrono` (the moved handler's stream/deadline machinery).

### Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- edit `server/src/modules/sync/handlers.rs` — was 191 lines; now a **thin
  shim**: `sync_router() = sync_routes::<ZieeIdentityResolver, SyncEntity>()`.
- edit `server/src/modules/sync/event.rs` — add `impl SyncSurface for SyncEntity`
  (`Principal=SyncConnPrincipal`, `Wire=SyncSseEvent`, `BaselinePerms=(ProfileRead,)`,
  `registry()`, `principal_user_id`, `connected_signal`, `recheck` — the latter
  byte-equivalent to the former inline re-check). `SyncEntity` + all wire types
  UNCHANGED.
- edit `server/src/modules/sync/registry.rs` — add `From<(User, Vec<Group>)> for
  SyncConnPrincipal` (the mount-site principal ctor); drop the now-unused
  `ClientConn`/`SYNC_CHANNEL_CAPACITY` re-exports (the moved handler consumed them).
- edit `server/src/modules/permissions/extractors.rs` — override
  `ZieeIdentityResolver::access_token_exp` (byte-identical to the former inline
  exp extraction).

### Stays app-side (the boundary)

`event.rs`'s concrete `SyncEntity`/`SyncAction`/`SyncEvent`/`SyncSseEvent` (all
`JsonSchema`, in the OpenAPI/`types.ts` surface), `publish` / `publish_session_to_users`
(name `SyncEntity`), the singleton `registry()`, and `SyncConnPrincipal`.

---

## Deliverable 2 — `ziee-onboarding` (CUT with move)

### Files move: INTO `ziee-onboarding` (new crate, submodule `sdk/`)

- new `crates/ziee-onboarding/src/models.rs` — `OnboardingProgress`. Moved
  BYTE-FOR-BYTE (0 `crate::` refs).
- new `crates/ziee-onboarding/src/repository.rs` — `OnboardingRepository`
  (3 methods, `query!`/`query_as!` over `user_onboarding` ONLY). ONE edit:
  `use crate::common::AppError` → `use ziee_core::AppError`.
- new `crates/ziee-onboarding/migrations/202607140195_onboarding_schema.sql` —
  the `user_onboarding` table, moved BYTE-FOR-BYTE (NO foreign keys → applies on a
  bare DB). Filename/content preserved → sqlx checksum + version preserved.
- new `crates/ziee-onboarding/src/lib.rs` — `pub mod models; pub mod repository;`
  + re-exports.
- new `crates/ziee-onboarding/build.rs` — provisions the onboarding-only build DB
  (`ziee_onboarding_build_<key>`) from its own `migrations/` via
  `ziee_build_support::provision_build_db` (its `query!` verify against it).
- new `crates/ziee-onboarding/Cargo.toml` — `ziee-core` + serde/schemars/sqlx/uuid;
  `[build-dependencies] ziee-build-support`.

### Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- del `server/src/modules/onboarding/{models,repository}.rs` (moved to crate).
- del `server/src/modules/onboarding/migrations/202607140195_onboarding_schema.sql`
  (moved to crate; the app globs it from `sdk/crates/ziee-onboarding/migrations/`).
- edit `server/src/modules/onboarding/mod.rs` — keeps `pub mod handlers; mod
  routes;` + registration; replaces `pub mod models; mod repository; pub use
  repository::OnboardingRepository;` with `pub use
  ziee_onboarding::{OnboardingRepository, models};` so `Repos.onboarding`,
  `super::models::OnboardingProgress` (handlers), and the OpenAPI schema resolve.
- edit `server/Cargo.toml` — add the `ziee-onboarding` path dep.
- edit `server/build.rs` — add `ziee-onboarding/migrations` to the explicit
  `sdk_crate_migration_dirs` list.

### Stays app-side (the boundary)

`onboarding/handlers.rs` (names `SyncEntity::Onboarding` / `publish` / `Repos` /
`RequirePermissions<(ProfileEdit,)>` / `JwtAuth` / `SyncOrigin` + the id-validator
+ its `#[cfg(test)]` units), `routes.rs`, the `AppModule` registration, and the
`202607144185_onboarding_fkeys.sql` FK migration (`user_id → users(id)`,
domain-coupled). This mirrors `ziee-notification`'s handler/route/sync boundary;
the ONE divergence is the repository + build DB, which onboarding's self-contained
queries let it own.

---

## Symbols moved

- Deliverable 1: `subscribe_sync` handler body + docs (now `sync_routes`),
  `recheck_interval`, the `SYNC_RECHECK_TICK_MS` seam. New public:
  `sync_routes`, `SyncSurface`, `RecheckOutcome`, `IdentityResolver::access_token_exp`.
- Deliverable 2: `OnboardingProgress`, `OnboardingRepository`.
- Schemars keys preserved (`OnboardingProgress`, `SyncSseEvent`/`SyncEvent`/…) →
  OpenAPI byte-identical. Route path/permission strings preserved.
