# Chunk ziee-auth-routes — BOUNDARY

## What is now in the SDK (`ziee-auth`)
The COMPLETE auth surface — engine (BA-full) + HTTP (this chunk):
- `ziee-auth/src/auth/http/` (feature `routes`, default-on): `handlers`, `routes`
  (`auth_routes<R>` / `auth_admin_routes<R>`), `jwt_extractor` (`JwtAuth`/`OptionalJwtAuth`),
  `session_settings` (GET/PUT).
- `ziee-auth/src/auth/permissions.rs`: `AuthProvidersRead/Manage`, `SessionSettingsRead/Manage`.
- `ziee-auth/src/user/permissions.rs`: `ProfileRead`, `ProfileEdit`.

A second app mounts working auth by:
```rust
router
  .nest("/auth", ziee_auth::auth::auth_routes::<MyResolver>())
  .merge(ziee_auth::auth::auth_admin_routes::<MyResolver>())
```
supplying `Extension<AuthContext>` (pool + its `AuthEventSink`/`AuthSyncSink`),
`Arc<JwtService>`, and `Arc<MyResolver>` (its `IdentityResolver`) as extension layers,
plus `set_trust_forwarded_headers(bool)` at boot. `MyResolver` must resolve to
ziee-auth's own `User`/`Group` wire types (the associated-type bound), because every
auth response returns them.

## What STAYS app-side (config + domain wiring, not mechanism)
- The concrete `SyncEntity`/`SyncAction` enums (JsonSchema codegen contract) + the
  `PublishSyncSink`/`EventBus`-backed `AuthSyncSink`/`AuthEventSink` impls.
- `ZieeIdentityResolver` (ziee's JWT+Repos-backed `IdentityResolver`) + the boot
  extension layering (`AuthContext`, `JwtService`, resolver) in `lib.rs`/`main.rs`.
- `AuthModule` registration + `init()` (config seed, expiry-cleanup loop,
  trust-flag install).
- The user-ADMIN handlers/permissions (`UsersRead`/`GroupsRead`/…, `delete_user`
  cascade) — they gate app-domain cleanup (skill/file/hub), per BA-full C3.
- Which providers are enabled, client secrets, redirect URLs, branding — all config
  (`auth_providers` table rows + `trust_forwarded_headers`), injected, never code.

## Residual coupling / follow-ups (NOT this chunk)
- **MIGRATE-squash harness debt** (pre-existing, blocks the integration suite):
  `tests/common/harness_inner.rs::template_migration_dirs()` must point at
  `migrations-merged` (not the deleted flat `migrations`), and
  `tests/{hub,code_sandbox}/*` must stop `include_str!`-ing squash-deleted numeric
  migration filenames. Owned by a MIGRATE-squash follow-up, not ziee-auth-routes.
- **Engine-only feature** — `routes` is default-on; no engine-only consumer exists
  yet, but `--no-default-features` yields a lean aide/axum-free engine build.
- The `ziee://` / desktop tunnel_auth `change_password` remain desktop-crate-owned
  (separate route, untouched).

## Boundary invariants held
- E11 skeleton (framework-only guard) does NOT depend on ziee-auth → unaffected;
  builds framework-only with no build DB.
- Auth-only build DB (`ziee_auth_build_<key>`) unchanged — the routes have 0 `query!`
  macros.
- N2 golden byte/canonical-identical on BOTH surfaces.
