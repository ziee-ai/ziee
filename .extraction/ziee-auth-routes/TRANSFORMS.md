# Chunk ziee-auth-routes — TRANSFORMS (design-gate resolutions, zero TBD)

## Decision 1 — N2 byte-identical OpenAPI under the crate boundary (the hard gate)
The auth handlers are aide-typed; moving them must not change `openapi.json` /
`types.ts` on EITHER surface.

**Resolution:** MOVE, not rewrite. Four independent reasons the spec is stable:
- **Schema DTOs** (`User`, `Group`, `LoginResponse`, `MeResponse`, `AuthProviderResponse`,
  `SessionSettings`, …) already live in `ziee-auth` (BA-full / BA-spike). schemars
  keys a schema by the type's SHORT ident, not its crate path, so every `$ref` +
  schema NAME is byte-stable.
- **`with_permission::<Perms>`** emits `PermissionDetail { name, value, description }`
  from each permission type's `NAME`/`PERMISSION`/`DESCRIPTION` CONST strings. The
  moved permission types (`AuthProviders*`, `SessionSettings*`, `Profile*`) carry
  those consts byte-identical, so the 403 schema + description are unchanged
  regardless of where the type is defined.
- **`RequirePermissions<R, Perms>`** derives `OperationIo` with `#[aide(input)]`
  and NO request-body schema, so its OpenAPI contribution is empty and independent
  of the resolver `R`. Making handlers generic over `R` therefore cannot move the spec.
- **operationIds** (`Auth.login`, `AuthProviders.testConfig`, …) are string literals
  in the moved `*_docs` fns, carried verbatim.

**SPIKE result (before commit):** regen ui + desktop → `types.ui.ts` BYTE-IDENTICAL,
`types.desktop.ts` BYTE-IDENTICAL, `openapi.ui.json` CANONICALLY-EQUAL,
`openapi.desktop.json` CANONICALLY-EQUAL vs `.extraction/baseline`. Generated files
`git checkout`-restored after. **N2 gate GREEN.**

## Decision 2 — Sync emission without a ziee-domain `SyncEntity` in the SDK
The handlers publish auth entities (User/Group/Profile/Session/SessionSettings/
AuthProvider). The concrete `SyncEntity`/`SyncAction` enums derive `JsonSchema` and
must stay app-side (codegen contract).

**Resolution:** Already designed by BA-full — the moved code emits over the
crate-local `AuthSyncEntity` / `AuthSyncAction` abstractions through the injected
`AuthSyncSink` trait (`ziee_auth::auth::context`), with `Audience` = the framework
`ziee_framework::sync::Audience` and the origin from `ziee_framework::sync::SyncOrigin`.
The app installs a `PublishSyncSink` that maps `AuthSyncEntity::User → SyncEntity::User`
etc. before the real `publish`. Chosen path = **(a) auth-owned abstract entities**
(the SDK crate never names a ziee-domain enum variant); option (b), a per-emit seam,
was unnecessary because (a) already exists and is byte-behaviour-identical (same
events, same audiences). The moved handlers name `AuthSyncEntity::{Profile,
SessionSettings, AuthProvider}` + `AuthSyncAction::{Create,Update,Delete}` only —
zero ziee-domain sync types cross the boundary. **No STOP condition hit.**

## Decision 3 — Config injection (enabled providers / redirect base / branding)
Must come from injected config, not a ziee global.

**Resolution:** No new config struct was needed — the surface already reads config
through injected seams:
- **Enabled providers** come from the ziee-auth-owned `auth_providers` DB table
  (via `ctx.pool()` on the injected `AuthContext`), not a ziee global.
- **OAuth redirect_uri** is derived at request time from the `Host` /
  `X-Forwarded-Host` / `X-Forwarded-Proto` headers, gated by the
  `trust_forwarded_headers` static that the app installs at boot via
  `ziee_auth::auth::set_trust_forwarded_headers(config.server.trust_forwarded_headers)`
  (unchanged in `AuthModule::init`). That single config value is the injected seam.
- **Branding** — the auth surface returns no branding; the login page renders it
  client-side. Nothing to inject.
- Verified by grep: the moved handlers name ZERO `crate::core` / `Config` / `Repos`
  / `get_app_data_dir` / `url_validator` / `AppEvent` / `EventBus` symbols (the six
  BG globals + Config are all gone). The only injected inputs are `Extension<AuthContext>`,
  `Arc<JwtService>`, `Arc<R>` (resolver) — all app-installed extension layers.

## Sub-decision — identity pluggability without breaking equivalence (N1)
The framework `RequirePermissions<R, Perms>` names `R::User`/`R::Group`; the profile
handlers touch the concrete `User` (`.id`, `.password_hash`).

**Resolution:** The route bundle is generic over `R: IdentityResolver<User = User,
Group = Group>` — the auth MECHANISM (JWT validation, group loading) stays pluggable
(N1), while `User`/`Group` are FIXED to ziee-auth's own wire types (the crate owns
them; every auth response returns them). ziee mounts with `ZieeIdentityResolver`; a
second app mounts with its own resolver. Non-permission handlers
(register/login/refresh/logout/me/oauth/providers/link-account — `me`/`logout` use
the concrete `JwtAuth`) stay non-generic. `#[debug_handler]` is dropped from the 10
generic handlers (it rejects generics; it is a compile-time diagnostic only, no
runtime/OpenAPI effect).

## Sub-decision — `routes` cargo feature (engine/surface split)
**Resolution:** `default = ["routes"]`, `routes = ["dep:aide", "dep:axum"]`, aide/axum
optional. ziee (default features) gets the surface; an engine-only consumer builds
`--no-default-features`. The always-on engine code is axum-free (only `axum-login`),
so the gate is clean. The routes have no `query!` macros → the auth-only build-DB
lane is unaffected. The E11 skeleton (framework-only guard) does NOT depend on
ziee-auth, so it is untouched (confirmed: `cargo check -p skeleton-server` green with
no DATABASE_URL).
