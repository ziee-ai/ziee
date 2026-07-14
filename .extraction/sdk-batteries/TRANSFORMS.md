# Chunk sdk-batteries — TRANSFORMS (design-gate resolutions, zero TBD)

- **T-1** `worktree_db` (module): MOVED verbatim ziee `build_helper/` → SDK
  `ziee-build-support/src/worktree_db.rs`. — **why:** decision #11 makes the
  per-worktree keying an SDK contract shared by BOTH apps' `build.rs`; a copied
  file is drift-prone (it was already duplicated inside `ziee-auth/build.rs`). Only
  the module doc-comment gained a provenance line; every symbol + all 5 unit tests
  are byte-for-byte the same, so the derived `ziee_build_<key>` is identical.
- **T-2** `compose_merged_migrations`: MOVED body → `…::migrations`, **parameterized**
  on `(merged_dir, module_roots)`. — **why:** the SDK crate can't `env!("CARGO_MANIFEST_DIR")`
  (that would be the SDK's dir); parameterizing lets ziee pass its own manifest-derived
  roots so the glob→sort→dedup→copy produces a byte-identical `migrations-merged`.
- **T-3** `ensure_build_db`: MOVED the inline admin-connect/CREATE block. — **why:**
  same provisioning, now callable; preserves the swallowed duplicate-database race +
  the fall-back-to-base-on-unreachable path so a two-worktree concurrent build is
  still safe.
- **T-4** `embedded_pg` installation_dir/data_dir: `.expect("… Config::resolve_paths")`
  → `.clone().unwrap_or_else(|| default_embedded_dir(..))`. — **why:** P0-b. The panic
  string named a ziee-app-side symbol (boundary LEAK) and the hard `expect` forced a
  fresh app to hard-code both dirs. Now they default to `<app data dir>/postgres[-data]`
  (the same convention ziee's `resolve_paths` uses). ziee always fills them → its
  `Some` wins → byte-identical for ziee.
- **T-5** `DefaultIdentityResolver`: NEW impl of `IdentityResolver<User,Group>` over
  `UserRepository` + `Arc<JwtService>`. — **why:** G1. Behaviour matches ziee's
  `ZieeIdentityResolver` (JWT validate → load user → inactive-user 403 → load groups)
  but pool-bound instead of global-`Repos`-bound, so an app needs no app-global.
- **T-6** `mount_auth::<R>` + `AuthMountOptions`: NEW one-call mounts. — **why:** G1
  concrete. Folds the 4 things every app hand-wrote: install `Extension<AuthContext>`
  + `Extension<Arc<JwtService>>` + `Extension<Arc<R>>`, `set_trust_forwarded_headers`,
  the session-settings seed, and nest `auth_routes::<R>` + `auth_admin_routes::<R>`.
- **T-7** `Noop{Event,Sync}Sink`: NEW no-op sink impls. — **why:** G1 — the two trait
  impls `AuthContext::new` requires that every app copied verbatim.
- **T-8** `ServerConfig::{load_from,discover,load,validate_jwt_secret}`: NEW. — **why:**
  G2. Additive loader with env-`CONFIG_FILE`-first discovery + weak-`jwt.secret`
  refusal (min 32 + placeholder denylist, mirrors ziee-auth `jwt::try_new`). Unknown
  app-specific YAML keys are ignored (serde default), so it parses a real app config.
- **T-9** `serve(listener, router)`: NEW. — **why:** P1 / G7 — bakes in
  `into_make_service_with_connect_info::<SocketAddr>()` (else tower-governor errors
  "Unable To Extract Key!") + graceful shutdown.
- **T-10** CORS permissive `SECURITY:` ERROR → `debug` on loopback bind. — **why:** P1
  — the loud ERROR fired on every localhost dev boot; a public (non-loopback) bind
  still gets the full ERROR so a real misconfig is caught.

## Decision 1 — build_support home: crate (#11) vs `ziee_framework::build_support` (brief)
**Resolution:** BOTH. `ziee-build-support` is a lean, standalone SDK **crate**
(decision #11's "SDK build-support crate, shared by both apps' build.rs"); ziee-framework
`pub use ziee_build_support as build_support`, giving the exact
`ziee_framework::build_support::{compose_merged_migrations, provision_build_db, worktree_db}`
path the brief names. Each app's `build.rs` depends on the LEAN crate directly
(`[build-dependencies] ziee-build-support`) rather than dragging the whole framework
(aide/axum/reqwest/postgresql_embedded) into the build graph — so it's cheap. The
crate is build-DB-free (sqlx runtime only, no `query!`), satisfying "usable as a
build-dependency; keep it build-DB-free".

## Decision 2 — build_support equivalence (the hard gate)
The moved `migrations-merged` MUST be byte-identical and the build DB MUST provision
exactly as before. **Resolution + proof:** the compose logic is a pure, deterministic
file operation (glob `*/migrations/*.sql` under the two roots → sort → basename-dedup →
copy); the source migration files are unchanged by this chunk (`git diff` touches zero
`*.sql`). I proved it by fingerprinting: `sha256(sorted concat of all module-owned
source `*.sql`) == sha256(sorted concat of the built `migrations-merged/*.sql`)` — i.e.
`migrations-merged` is the exact union, regardless of which code produced it. The
per-worktree DB name (`ziee_build_<worktree_key>`) is derived by the verbatim-moved
`worktree_key`, so it is identical; the golden regen rebuilds ziee → runs the new
`build.rs` → provisions + migrates that DB → emits the SAME openapi/types (E8 green).

## Decision 3 — mount_auth layering semantics (truly one-call)
`Router::layer` applies to routes present at layer time. **Resolution:** `mount_auth`
merges the auth routes then layers the three extensions on the merged router, and its
docs state the contract: call it AFTER registering the app's own permission-gated
routes (or serve the returned router), so `RequirePermissions` routes get the resolver
extension. This matches ziee (which layers the same extensions on the fully-built app)
and collapses CytoAnalyst's ~230 lines + 2 trait impls to ~5 lines.

## Decision 4 — provision_build_db runtime nesting
`provision_build_db` spins its OWN current-thread runtime (ergonomic for a plain
`fn main()`), so it must NOT be called from a `#[tokio::main]` build.rs. **Resolution:**
ziee's `build.rs` (already `#[tokio::main]`) uses the granular `ensure_build_db` (async)
+ owns its bespoke schema-wipe/multi-source migrate; only a fresh plain app uses the
one-call `provision_build_db`. Documented on both.
