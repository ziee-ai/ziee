# Chunk ziee-auth-routes — CUT manifest (decision N10)

Complete the auth extraction: move the auth HTTP/aide **surface** (the generic
auth MECHANISMS) out of ziee's `modules/auth/*` into a mountable, resolver-generic
routes bundle in `ziee-auth`, so a second app mounts working auth endpoints
instead of re-implementing them. Equivalence-preserving MOVE (not rewrite): byte
strings, error shapes, response types, and — critically — the OpenAPI spec are
preserved byte-for-byte on BOTH surfaces (N2).

## Files

### Moved (app → SDK), equivalence-preserving
- move: `src-app/server/src/modules/auth/handlers.rs` (2255 L) →
  `sdk/crates/ziee-auth/src/auth/http/handlers.rs` (2247 L). 84 changed lines,
  ALL mechanical: import-path rewrites (`crate::common`→`ziee_core`,
  `crate::modules::permissions`→`ziee_framework::permissions`,
  `crate::modules::user`→`crate::user`, `crate::modules::sync`→`ziee_framework::sync`,
  `super::X`→`crate::auth::X`); `token_response` `pub(crate)`→`pub` (ziee app shim
  re-export); and the genericity transform (10 permission-gated handlers gain
  `<R: IdentityResolver<User = User, Group = Group>>`, their `RequirePermissions<(P,)>`
  → `RequirePermissions<R, (P,)>`, and lose `#[debug_handler]` — a compile-time
  diagnostic macro that doesn't support generics; zero runtime/OpenAPI effect).
- move: `src-app/server/src/modules/auth/routes.rs` (84 L) →
  `sdk/crates/ziee-auth/src/auth/http/routes.rs` (101 L). Route builders
  `auth_routes` / `auth_admin_routes` gain `<R: IdentityResolver<User=User, Group=Group>>`
  and turbofish the generic handlers (`update_profile::<R>` …). Same paths, same order.
- move: `src-app/server/src/modules/auth/jwt_extractor.rs` (124 L) →
  `sdk/crates/ziee-auth/src/auth/http/jwt_extractor.rs` (124 L). Verbatim except
  2 import lines (`super::jwt`→`crate::auth::jwt`, `crate::common::AppError`→`ziee_core::AppError`).
- move: `src-app/server/src/modules/auth/session_settings.rs` (92 L) →
  `sdk/crates/ziee-auth/src/auth/http/session_settings.rs` (89 L). The GET/PUT
  REST handlers; 2 handlers gain `<R: IdentityResolver<User=User, Group=Group>>`;
  same validation ranges, same `AuthSyncEntity::SessionSettings` publish, same docs.
- move: `src-app/server/src/modules/auth/permissions.rs` (65 L) →
  `sdk/crates/ziee-auth/src/auth/permissions.rs` (65 L). The 4 auth-domain
  permission keys (`AuthProvidersRead/Manage`, `SessionSettingsRead/Manage`).
  Only import changed (`crate::modules::permissions::types::PermissionCheck`
  → `ziee_identity::PermissionCheck`). Const strings byte-identical.
- move (partial): the **Profile** permission family (`ProfileRead`, `ProfileEdit`)
  from `src-app/server/src/modules/user/permissions.rs` →
  `sdk/crates/ziee-auth/src/user/permissions.rs` (32 L). `/auth/profile` +
  `/auth/password` gate on `ProfileEdit`, so it travels with the surface. Const
  strings byte-identical. The user-ADMIN keys (`UsersRead`/`GroupsRead`/…) STAY
  app-side (they gate user-admin handlers that stayed app-side).

### New (SDK)
- new: `sdk/crates/ziee-auth/src/auth/http/mod.rs` — the routes-bundle module
  (feature-gated `routes`; re-exports `auth_routes`/`auth_admin_routes`).

### Edited
- edit: `sdk/crates/ziee-auth/Cargo.toml` — add `[features] default=["routes"]`,
  `routes=["dep:aide","dep:axum"]` + optional aide/axum (versions matched to
  ziee-framework). The routes surface has ZERO `query!` macros → the auth-only
  build-DB story is unchanged.
- edit: `sdk/crates/ziee-auth/src/auth/mod.rs` — `pub mod permissions;` +
  `#[cfg(feature="routes")] pub mod http;` + re-export `auth_routes`/`auth_admin_routes`.
- edit: `sdk/crates/ziee-auth/src/user/mod.rs` — `pub mod permissions;`.
- edit: `sdk/Cargo.lock` — aide/axum now under ziee-auth (unification).

### App-side → THIN CONSUMER (equivalence-preserving shims)
- edit: `src-app/server/src/modules/auth/mod.rs` — DELETE the 5 file-module
  declarations; add module-path shims (`handlers`/`jwt_extractor`/`permissions`/
  `session_settings` re-export from `ziee_auth`); re-export `auth_routes`/
  `auth_admin_routes`; `register_routes` mounts `auth_routes::<ZieeIdentityResolver>()`
  + `auth_admin_routes::<ZieeIdentityResolver>()`. The `init()` boot wiring
  (config seed, cleanup loop, `set_trust_forwarded_headers`) is UNCHANGED.
- edit: `src-app/server/src/modules/user/permissions.rs` — replace the two
  Profile struct defs with `pub use ziee_auth::user::permissions::{ProfileEdit, ProfileRead};`.
- delete: the 5 moved app-side files (handlers/routes/jwt_extractor/session_settings/permissions).

## Symbols
- moved handlers (18): `register`, `login`, `refresh`, `logout`, `me`,
  `update_profile`, `change_password`, `oauth_authorize`, `oauth_callback`,
  `oauth_callback_post`, `ensure_unique_username`, `link_account`,
  `list_public_providers`, `admin_{list,create,update,delete,test}_provider`,
  `admin_test_provider_config` (+ their `*_docs`), `token_response`.
- moved extractors: `JwtAuth`, `OptionalJwtAuth`.
- moved route builders: `auth_routes<R>`, `auth_admin_routes<R>`.
- moved permissions: `AuthProvidersRead/Manage`, `SessionSettingsRead/Manage`,
  `ProfileRead`, `ProfileEdit`.
- shim re-exports (paths preserved): `crate::modules::auth::{handlers::{token_response,
  ensure_unique_username}, jwt_extractor::{JwtAuth,OptionalJwtAuth}, permissions::*,
  session_settings::*, auth_routes, auth_admin_routes}`,
  `crate::modules::user::permissions::{ProfileRead, ProfileEdit}`.

## Design-gate (the three N10 gates — resolutions in TRANSFORMS.md)
1. **N2 byte-identical OpenAPI** — PASSED both surfaces (`types.{ui,desktop}.ts`
   byte-identical + `openapi.{ui,desktop}.json` canonically-equal vs baseline).
   Held by: schema DTOs already ziee-auth-owned (short-ident schemars keys);
   `with_permission` emits permission CONST strings (preserved), not type location;
   `RequirePermissions<R,_>` `OperationIo` is input-only (no schema, R-independent);
   operationIds are string literals carried verbatim.
2. **Sync emission** — already solved by BA-full's `AuthSyncEntity`/`AuthSyncAction`
   + `AuthSyncSink` seam; the moved handlers emit via `ctx.sync.publish(AuthSyncEntity::…)`
   + `ziee_framework::sync::{Audience, SyncOrigin}`. NO ziee-domain `SyncEntity` in the SDK.
3. **Config injection** — the OAuth redirect_uri derives from request headers +
   the `trust_forwarded_headers` static (installed at boot by the app's
   `set_trust_forwarded_headers`); enabled providers come from the ziee-auth-owned
   `auth_providers` table. NO ziee `Config`/`Repos` global is read. Confirmed by
   grep: the moved handlers name zero `crate::core`/`Config`/`Repos`/`url_validator`.
