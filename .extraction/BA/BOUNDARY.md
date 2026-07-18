# Chunk BA-full — BOUNDARY (exit checklist)

The auth CORE moved into `ziee-auth` (deps: `ziee-framework` + `ziee-identity`
+ `ziee-core`). Prerequisites resolved by BG (6 globals) + BG-2 (`secret` +
`url_validator` → `ziee-framework`). All 4 STOP_REPORT couplings handled.

## Gate results

| Gate | Result |
|---|---|
| `cargo check -p ziee` (lib + `ziee` bin) | **exit 0** (only pre-existing dead-code warnings: knowledge_base / notification / scheduler / mcp) |
| `cargo check -p ziee-desktop` | **exit 0** |
| `cd sdk && cargo check --workspace` (ziee-auth + auth-only build DB) | **exit 0** |
| `cargo check -p ziee-auth --tests` (moved in-source test modules) | **exit 0** |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL** vs baseline (and vs HEAD: `git checkout` updated 0 paths) |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL** |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL** (`jq -S`) |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL** |
| Regenerated openapi/types | restored via `git checkout` (were already identical) |

## EA — merged migrator + schema identity (Chunk BA gate)

- **Auth-only migration subset self-contained.** The 10 structural auth-table
  migrations `{1, 27, 44, 46, 47, 48, 64, 125, 129, 130}` moved into
  `sdk/crates/ziee-auth/migrations/` **byte-identical** (checksums preserved;
  `diff -q` on mig 1 = IDENTICAL). Applied ALONE to a FRESH db → **9 auth tables,
  zero errors, no dangling cross-table FK, no `pgcrypto` needed** (the provider
  repo treats `client_secret_encrypted` as a plain `BYTEA`; crypto is runtime
  `encrypt_secret`, never inside a `query!`). This is the auth-only build DB
  `ziee_auth_build_<key>` that `ziee-auth`'s own `build.rs` provisions on
  `:54321`; `cargo check -p ziee-auth` verifying **every** auth `query!` against
  it is the empirical proof the per-query migration audit is complete.
- **Merged migrator applies fresh AND produces the baseline schema.** `build.rs`
  composes `server/migrations-merged/` = the app's own `migrations/` (137) ∪
  `ziee-auth`'s `migrations/` (10) = **147**, version-sorted (identical to ziee's
  pre-extraction set). BOTH the runtime `sqlx::migrate!("./migrations-merged")`
  (`core/database/mod.rs`) and the build-DB provisioner (`build.rs`) use it; the
  desktop headless path points its server-migrator at `../../server/migrations-merged`.
  The real merged Migrator ran during `cargo check -p ziee` **without checksum
  errors**, and `pg_dump --schema-only` of the resulting build DB
  (`ziee_build_3533975a`, real sqlx Migrator over merged + desktop) is
  **byte-identical to `.extraction/baseline/schema.sql`** (94 tables; normalized
  diff = **0 lines**, normalizing only the per-dump `\restrict` token + version
  comments). Existing deployments are therefore unaffected (same version-sorted
  history, same final schema); fresh DBs get the identical result.
- **Security angle.** No secret material moved into the SDK build lane: the
  auth-only build DB carries only DDL, and the provider secret crypto stays a
  runtime `pgp_sym_encrypt` (framework `secret`), never a compile-time macro, so
  the auth-only DB needs no key + no pgcrypto. The `secret_key` is still injected
  per-process via `AuthContext` (BG seam), never embedded. `migrations-merged/`
  is a gitignored build artifact (no committed generated SQL).

## The FOUR STOP_REPORT couplings — how each was handled

- **#1 `common::secret`** → retargeted to `ziee_framework::secret::{encrypt_secret,
  resolve_optional_secret}` in `providers/repository.rs` (BG-2 landed it in the
  framework). No app-crate dep remains.
- **#2 `url_validator` / `core::outbound`** → retargeted to
  `ziee_framework::url_validator::{OutboundUrlPolicy, validate_outbound_url,
  build_validated_client}` in `providers/{oauth2,apple}.rs`. The app's now-dead
  `core/outbound.rs` re-export kept (`#[allow(unused_imports)]`).
- **#3 `delete_user` domain cascade** → **KEPT APP-SIDE**. `modules/user/handlers/*`
  (incl. the skill/file/hub pre-delete collection interleaved with the cascade
  DELETE) stay in ziee; only the user REPO/models/types/events/service moved.
  ziee-auth does NOT own user admin CRUD in v1 (the C3(a) resolution).
- **#4 `AuthSyncSink` names concrete `SyncEntity`/`SyncAction`** → in-move TRANSFORM.
  `context.rs` (now in ziee-auth) declares crate-local `AuthSyncEntity` /
  `AuthSyncAction`; the app-side `PublishSyncSink` (`core/events.rs`) maps them
  onto the real `SyncEntity`/`SyncAction`. The ~24 `ctx.sync.publish(...)` call
  sites (auth/handlers.rs ×5, session_settings ×1, user/handlers/groups.rs ×7+1,
  user/handlers/user.rs ×11) now pass the abstract enums. `SyncEntity` stays
  app-side (JsonSchema codegen contract) → golden byte-identical.

## Layered split (the 5th coupling, resolved as C3's sibling)

`auth/handlers.rs` (2255 lines), `routes.rs`, `mod.rs` (AuthModule registration),
`permissions.rs`, `jwt_extractor.rs`, and the session-settings REST handlers are
coupled to the app's `permissions` (`RequirePermissions`/`with_permission`/
`PermissionCheck`) + `module_api` + `aide` — none in scope. Moving them would drag
the RBAC + OpenAPI machinery into the SDK and threaten the golden. They **stay
app-side** (same resolution as C3 for user/handlers); ziee-auth owns everything
they call into. `session_settings.rs` was SPLIT: DTOs + repository (`query!`) →
ziee-auth; the permission-gated GET/PUT handlers → app-side.

## One extra coupling surfaced + handled
- `impl From<JwtConfig> for JwtSettings` (was `core/config.rs`) became an
  orphan-rule violation once `JwtSettings` moved. Relocated into
  `ziee-auth::auth::jwt` (`impl From<ziee_core::config::JwtConfig> for JwtSettings`
  — legal: `JwtSettings` local, `ziee-core` a dep). All `config.jwt.into()` sites
  unchanged.
- `trust_forwarded_headers()` (OnceCell in the app auth/mod.rs, read by the moved
  `cookie.rs`) → the global moved into `ziee-auth::auth` (`OnceLock` + getter +
  `set_trust_forwarded_headers`); app `AuthModule::init` calls the setter and
  re-exports the getter.

## Committed (SDK submodule only)
Per the BG/BG-2 convention, the SDK side (`ziee-auth` Cargo.toml/build.rs/lib.rs
+ `migrations/` + `src/auth/*` + `src/user/*` + Cargo.lock) is committed in
`sdk/` — NOT pushed. The ziee app side (auth/user mod shims, handlers #4 transform,
session_settings split, core/{events,config,outbound,database}, build.rs merged
composer, .gitignore, 10 removed app migrations, desktop lib.rs, src-app/Cargo.lock)
is left uncommitted for the orchestrator. No remote pushed; `/data/pbya/ziee/ziee`
untouched.
