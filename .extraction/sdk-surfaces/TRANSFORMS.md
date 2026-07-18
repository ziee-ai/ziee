# Chunk `sdk-surfaces` — TRANSFORMS

Every transform, each with its decision + resolution. Zero TBD.

## T-1 — `subscribe_sync` handler → generic `sync_routes::<R, S>()`

### Decision — a `SyncSurface` seam alongside the resolver, NOT a widened `SyncEntityKind`

The handler named four ziee concretes the framework must not: (a) the
`RequirePermissions<(ProfileRead,)>` gate, (b) `Extension<Arc<JwtService>>` for
the `exp`, (c) `Repos`-backed re-check, (d) `SyncConnPrincipal` / `registry()` /
`SyncSseEvent`. `SyncEntityKind` (the existing entity seam) is `session_signal`
only, and the framework's registry tests carry a MINIMAL `TestEntity` impl — so
widening `SyncEntityKind` would break that mock. So the routing needs are bundled
into a NEW `SyncSurface` trait (its own file, `sync/routes.rs`), leaving
`SyncEntityKind` + `deliver_session_to_users<E: SyncEntityKind>` + `TestEntity`
untouched.

**Resolution:** `sync_routes::<R, S>()` where `R: IdentityResolver`, `S:
SyncSurface`, `S::Principal: From<(R::User, Vec<R::Group>)>`. `SyncSurface` bundles
`type {Principal, Wire, BaselinePerms}` + `fn {registry, principal_user_id,
connected_signal}` + `async fn recheck`. The `tokio::select!` loop body,
`recheck_interval()`, the `ConnGuard` drop-unregister, and the handshake are moved
verbatim; only the four concretes above are read through `R`/`S`. ziee mounts
`sync_routes::<ZieeIdentityResolver, SyncEntity>()` — a 3-line `handlers.rs`.

## T-2 — the JWT `exp` seam: `IdentityResolver::access_token_exp`

### Decision — the resolver surfaces `exp`, not a new type param / verifier extension

The stream deadline needs the access token's `exp`. The framework's
`ziee_identity::TokenVerifier` has an opaque `Claims` (no `.exp` accessor), and
the concrete `JwtService` is only reachable as `Extension<Arc<JwtService>>` — a
type the framework can't name. Adding a `V: TokenVerifier` param to `sync_routes`
would leak the JWT scheme into the signature.

**Resolution:** added `IdentityResolver::access_token_exp(&self, &Parts) ->
Option<i64>` with a default `None`. The resolver ALREADY owns token verification
(it validates the access token in `authenticate`), so exposing its `exp` is a
minimal, additive extension (default-`None` → non-breaking for any other resolver;
`ziee-auth` does not use it). A framework `AccessTokenExp<R>` extractor pulls the
installed `Arc<R>` from extensions and delegates; ziee's override is byte-identical
to the former inline extraction (`Arc<JwtService>` → `extract_token_from_header` →
`validate_access_token` → `.exp`). Keeps `sync_routes::<R, S>()` at two type params.

## T-3 — the periodic re-check → `SyncSurface::recheck`

### Decision — the whole `Repos`+baseline re-check is one app method returning a 3-way outcome

The re-check reloads the user by id (`Repos`), checks `is_active`, loads groups
(admin → empty), re-checks the baseline `profile::read` (`check_permission_union`),
and either refreshes the snapshot, tears down, or (on a DB blip) keeps the stream.
All four touch points are ziee-specific.

**Resolution:** `async fn recheck(user_id) -> RecheckOutcome<Self::Principal>`
(`Refresh(P)` / `TearDown` / `Transient`) — a faithful 1:1 of the former three
match arms. The framework loop maps them to `registry().refresh` / `break` / no-op,
byte-equivalent to the inline version. ziee's impl (in `event.rs`) calls `Repos` +
`check_permission_union("profile::read")` verbatim.

## T-4 — the mount-site principal ctor: `From<(User, Vec<Group>)>`

### Decision — link `R` to `S::Principal` at the mount site via a `From` bound

The handler built `SyncConnPrincipal { user, groups }` from the auth extractor's
`user`+`groups`. Generically that is `S::Principal` from `(R::User, Vec<R::Group>)`.

**Resolution:** the `sync_routes` where-clause requires `S::Principal:
From<(R::User, Vec<R::Group>)>`; ziee adds `impl From<(User, Vec<Group>)> for
SyncConnPrincipal`. The registry key `user_id` is read back via
`S::principal_user_id(&principal)` (ziee: `principal.user.id`) — `Principal` has no
id accessor, so this is a surface method rather than a `Principal` widening.

## T-5 — `OnboardingRepository` moved to the crate (with its OWN build DB)

### Decision — onboarding's self-contained queries let the REPOSITORY move (unlike notification)

`ziee-notification` kept its repository app-side because its FKs reference other
modules' tables, so a standalone crate build DB would fail them. Onboarding's
`query!`/`query_as!` touch `user_onboarding` ONLY — no cross-table FK is needed to
verify them — so the repository CAN own a self-sufficient build DB.

**Resolution:** `repository.rs` moved with ONE edit (`crate::common::AppError` →
`ziee_core::AppError` — same type, ziee re-exports it). The crate's `build.rs`
provisions `ziee_onboarding_build_<key>` from its own `migrations/` (the table
only) via `ziee_build_support::provision_build_db`; the `query!` verify against it.
`cargo check -p ziee-onboarding` exit 0 proves the queries match the moved schema
standalone.

## T-6 — the migration split: table → crate, FK → ziee (standalone-apply gate)

### Decision — crate migration is domain-FK-free; the `users` FK stays app-side

The standalone-apply gate requires each SDK crate's `migrations/` to apply on a
bare DB (0 domain FKs). The `user_onboarding` schema already had NO FK inline (the
`user_id → users(id)` FK lived in a SEPARATE `_onboarding_fkeys.sql`).

**Resolution:** moved `202607140195_onboarding_schema.sql` (table only) to the
crate BYTE-FOR-BYTE (checksum/version preserved); left
`202607144185_onboarding_fkeys.sql` in ziee's module migrations. The app globs
BOTH into the merged set (version-sorted: table `…0195` before FK `…4185`, which
sorts after `users` exists), so the merged schema is identical. Added
`ziee-onboarding/migrations` to `server/build.rs`'s explicit `sdk_crate_migration_dirs`
and to the `standalone_apply_gate.sh` CRATES table (dep = `ziee-identity`, ships no
migrations → applies alone).

## T-7 — `onboarding/mod.rs` re-export shim + retained files

### Decision — handlers/routes/registration stay in ziee (domain-coupled, mirror notification)

`handlers.rs` names `SyncEntity::Onboarding` / `publish` / `Repos` /
`RequirePermissions<(ProfileEdit,)>` / `JwtAuth` / `SyncOrigin` — the same reasons
notification's handlers stayed. Moving them would require a rewrite (injected
notifier + repo + generic perms), NOT an equivalence-preserving move.

**Resolution:** `mod.rs` keeps `pub mod handlers; mod routes;` + registration and
does `pub use ziee_onboarding::{OnboardingRepository, models};` so `Repos.onboarding`
(`core/repository.rs:50`) + `super::models::OnboardingProgress` (handlers) resolve.
The handler bodies + the `is_valid_onboarding_id` unit tests are byte-unchanged.
