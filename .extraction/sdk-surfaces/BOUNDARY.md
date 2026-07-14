# Chunk `sdk-surfaces` — BOUNDARY

What each moved surface may and may not name.

## Deliverable 1 — `ziee_framework::sync::routes`

- MAY name: `aide`/`axum` (the SSE + router surface), `futures-util` +
  `async-stream` + `tokio::time` (the stream/select machinery), `chrono` (the
  `exp` deadline arithmetic), `ziee_core::{ApiResult, AppError}`,
  `ziee_identity::{Principal, PermissionList}`, and the framework's own
  `IdentityResolver` / `RequirePermissions` / `with_permission` / `SyncRegistry`.
- MAY NOT name (and does not — grep-confirmed): ziee's `SyncEntity`,
  `SyncSseEvent`, `SyncConnectedData`, `SyncConnPrincipal`, `JwtService`, `Repos`,
  `check_permission_union`, `ProfileRead`, or `crate::modules::*`. Every one is
  reached through the `R: IdentityResolver` / `S: SyncSurface` seams. The wire enum
  is opaque as `S::Wire` (only the 200 schema); the principal opaque as
  `S::Principal`.
- The load-bearing line: the schema-bearing wire types (`SyncSseEvent` &c., in the
  OpenAPI/`types.ts` surface) STAY concrete in ziee, so the serialized schema + the
  generated `sync:<entity>` TS vocabulary are byte-unchanged. A second app defines
  its own `SyncSurface` (its own wire enum + registry singleton) and mounts
  `sync_routes()` — full cross-device sync, backend + the already-extracted
  frontend client.
- Security: the SSE auth-gate is PRESERVED. Subscribe is `RequirePermissions<R,
  S::BaselinePerms>` (ziee: `profile::read`); the 60s re-check re-resolves
  `is_active` + the baseline perm and tears the stream down on loss; the `exp`
  deadline bounds the stream; the handshake carries only a connection id
  (notify-and-refetch — no row data crosses). None of this weakened in the move.

## Deliverable 2 — `ziee-onboarding` (schema-bound, self-contained)

- MAY name: `ziee-core` (`AppError`), serde/schemars (the wire type), sqlx
  (`query!`/`query_as!` over `user_onboarding`), uuid. `[build-dependencies]`
  `ziee-build-support` (the build-DB provisioner).
- MAY NOT name (grep-confirmed): `crate::modules`, `SyncEntity`, `Repos`,
  `RequirePermissions`, `JwtAuth`. None appear.
- `migrations/` = ONE file creating `user_onboarding` with `user_id` a PLAIN
  NOT-NULL uuid column, NO foreign key (`REFERENCES public.` → EMPTY). Declared
  deps: `ziee-core` (no migrations) — so the crate's migration closure = its own
  one file → applies on a bare DB (standalone-apply gate).
- The ONE divergence from `ziee-notification`: onboarding OWNS its repository +
  a build DB, because its `query!` touch only `user_onboarding` (self-contained,
  ziee-auth-style). The domain coupling (the `user_id → users(id)` FK) lives
  ENTIRELY ziee-side (`server/src/modules/onboarding/migrations/*_onboarding_fkeys.sql`),
  sorted after both `user_onboarding` and `users` exist in the merged set.
- Stays app-side (domain-coupled, mirror notification): `handlers.rs`
  (`SyncEntity::Onboarding` / `Repos` / `RequirePermissions` / `JwtAuth`),
  `routes.rs`, the `AppModule` registration.

## E-gates (this chunk)

- **E (cargo):** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk &&
  cargo check --workspace` = 0.
- **Standalone-apply:** `ziee-onboarding/migrations` apply alone on a fresh DB (0
  domain FKs) — the new `standalone_apply_gate.sh` row.
- **Golden (BOTH surfaces):** `types.{ui,desktop}.ts` BYTE-IDENTICAL;
  `openapi.{ui,desktop}.json` CANONICALLY-EQUAL (jq -S) vs `.extraction/baseline/`.
  Both endpoints' path/response/permission are unchanged (Sync.subscribe moves the
  handler; onboarding moves whole).
- **Schema fingerprint:** UNCHANGED vs `baseline/schema.fp` (onboarding's table +
  FK move crate/stay app-side but the MERGED schema is identical).
