# Chunk BG-3 — TRANSFORMS (non-byte-identical changes + rationale)

Every changed line vs a verbatim copy, with its design decision + resolution.
Zero `TBD` / `TODO` / `ASK`. All transforms are equivalence-preserving; the E8
golden (types.ts byte-identical + openapi.json canonical, BOTH surfaces) is the
machine proof that no wire surface moved.

---

## Decision 1 — embedded-PG's SDK home is `ziee-framework::embedded_pg`; parameterize the 3 app-bound seams

The embedded/external PG bring-up is **generic infra** (build Settings, stop
existing, setup/start/stop, connect-with-retry, keep-alive + cleanup) EXCEPT
three app-/schema-bound touch-points that cannot move into an app-agnostic crate.

**Resolution:** move the generic lifecycle to `ziee-framework::embedded_pg` and
parameterize the three seams; ziee's `core::database` passes them in:

- **the migrator** — the original `sqlx::migrate!("./migrations-merged")
  .set_ignore_missing(true).run(&pool)` is schema-bound (verifies against ziee's
  merged migration dir). The framework takes `migrator: &'static Migrator` and
  calls `migrator.run(&pool)`. ziee owns `static MERGED_MIGRATOR:
  LazyLock<Migrator>` (macro-expanded + `set_ignore_missing(true)` applied once)
  and passes `LazyLock::force(&MERGED_MIGRATOR)` (a `&'static Migrator`). Same
  migration set, same ignore-missing semantics. (Runtime `Migrator`
  concatenation isn't a supported sqlx API — plan decision N3 — but the app's
  migrator was ALREADY the build-time-composed `migrations-merged` dir, so this
  is a straight pass-through, not a new composition.)
- **the Postgres binary version** — the original read `const POSTGRES_VERSION =
  env!("ZIEE_POSTGRES_VERSION")` (set by ziee's `.cargo/config.toml`) for the
  versioned `pg_ctl` stop path. The **standalone SDK workspace has no such env**,
  so the framework can't `env!` it. The framework takes `pg_ctl_version: String`;
  ziee passes `POSTGRES_VERSION.to_string()` (the const stays app-side).
- **pgvector install + smoke-test** — the original inlined
  `pgvector_install::{has_real_artifacts,install_into}` (after `setup()`, before
  `start()`) + `CREATE EXTENSION vector` + `mark_available` (after `start()`).
  `pgvector_install` is ziee-memory-specific (build.rs `OUT_DIR` include), so it
  stays app-side. The framework takes `EmbeddedPgHooks { after_setup:
  fn(&Path), smoke_test: fn(String)->BoxFuture }` — plain `fn` pointers (no
  captured state, so they survive the multi-attempt init retry). ziee fills them
  with the identical install + smoke logic + log strings. Call ordering is
  preserved exactly (setup → after_setup → start → smoke_test.await → url →
  store instance → register cleanup).

Everything else in `embedded_pg.rs` is byte-verbatim (Settings population, the
5-attempt retry, the 10-attempt connect backoff, the `DATABASE_POOL` /
`POSTGRESQL_INSTANCE` OnceCells, `cleanup_database`, the panic-hook + Drop
cleanup with the `Handle::try_current` double-fault fix, all `println!`/
`eprintln!`/`tracing` strings).

## Decision 2 — `config: &Config` → `pg: PostgreSqlConfig` + `external_url: String`

The framework can't name ziee's monolithic `Config`. But the entire
`PostgreSqlConfig` (incl. `Embedded`/`External`/`Pool`/`LoggingConfigPostgres`)
and `database_url()` **already live in `ziee-core::config::ServerConfig`** (moved
in B2). 

**Resolution:** the framework takes `pg: PostgreSqlConfig` (owned, cloned once at
the call site — matching the original's `config_clone`) + `external_url: String`
(ziee passes `config.database_url()`, used only on the external branch). Reads of
`config.postgresql.{use_embedded,embedded,external,pool}` become `pg.{…}`;
`config.database_url()` (external branch) becomes `external_url`. Same values.

## Decision 3 — the desktop embed-server boundary is the harness `ServerBoot` seam (app-side impl)

`ziee::start_server_with_routes` → `setup_server` is the app's ENTIRE
non-agnostic server assembly (`Repos` init, `create_modules`,
`ZieeIdentityResolver`, `build_auth_context`, control-mcp catalog). It cannot
move into the reusable harness.

**Resolution:** ziee-desktop implements the harness-defined
`ziee_desktop_harness::boot::ServerBoot` as `ZieeServerBoot`. Its `boot()`
contains the EXACT route-builder closure the live `start_backend_server` had
(`init_desktop_repositories`, merge desktop routes + re-apply CORS +
`Extension(jwt)`, dev Vite-proxy / prod static fallback), wrapping
`start_server_with_routes`, and returns `BootHandle{addr,pool,jwt}`. `shutdown()`
= `ziee::cleanup_server()`. The single behavioural delta vs the old closure: the
JWT service is captured OUT (via a boot-local `OnceLock`) into the `BootHandle`
instead of being written to the module `JWT_SERVICE` static inside the closure —
see Decision 4. `ApiRouter`/handlers live behind `Mutex<Option<..>>` (consumed
once; guard dropped before the first await so the boot future stays `Send`).

## Decision 4 — thread `BootHandle.{pool,jwt}`; JWT/pool stashes set from the handle, not the closure

The live boot + the `auto_login` command reached `ziee::Repos.{pool,user,app}` +
a closure-set `JWT_SERVICE` directly. D-full moves those consumers into the
harness, which cannot name `ziee::`.

**Resolution:** `start_backend_server` routes through `ZieeServerBoot::boot()`,
then threads the returned handle:
- `JWT_SERVICE` + a NEW `SERVER_POOL` static are set from `handle.jwt`/
  `handle.pool` (right after boot returns, before any post-boot step) — the
  `auto_login` command reads both stashes. Previously `JWT_SERVICE` was set
  inside the route-builder closure (during `setup_server`, before serve spawned)
  and `auto_login` used `ziee::Repos.pool()`. The stashes are now set a few
  statements LATER (after serve spawns) but still long before the window is
  created (`set_ready` → `create_main_window`), and `auto_login` is only invoked
  by the frontend AFTER the window loads — so there is no observable ordering
  change. This removes the last `ziee::Repos` read from the `auto_login` path.
- `run_desktop_migrations` / `enable_memory_admin_default` /
  `ensure_desktop_admin` take `&PgPool` (= `handle.pool` = `Repos.pool()`).
- `mint_admin_login(pool, jwt)` / `ensure_desktop_admin(pool)` use
  `UserRepository::new(pool)` / `AppRepository::new(pool)` — the SAME repository
  types `Repos.user` / `Repos.app` are built from the SAME pool, so
  `get_by_username` / `has_admin` / `create_admin_user` / `mint_session_tokens`
  are byte-behaviourally identical. Error strings are preserved verbatim
  ("Failed to get admin: …", "Admin not found - server may still be starting",
  "Failed to generate tokens: …", "Failed to check/create admin: …").

The app-side helpers that STAY app-side under D-full (migrations, backfill,
memory-default, owner-create) keep running in `start_backend_server`; only the
`ziee::Repos.{pool,user,app}` reads on the paths D-full relocates (owner
read/mint + the `auto_login` command) are fully de-globalized.
`backfill_system_mcp_assignments` (in `mcp/event_handlers`, out of the
`auth/*` + `backend/mod.rs` scope, and app-side under D-full) is left byte-
unchanged.

## Decision 5 — `AppRepository` + `UserRepository` re-exported from `ziee`

The threaded desktop consumer needs `UserRepository::new(pool)` (owner read) +
`AppRepository::new(pool)` (owner-create domain CRUD, kept app-side by BA), but
`ziee` only re-exported the global `Repos`.

**Resolution:** `pub use modules::{app::AppRepository, user::UserRepository}` in
`server/lib.rs`. `UserRepository` = `ziee_auth::user::UserRepository` (the same
type `Repos.user` is); `AppRepository` = the app-side domain repo (the same type
`Repos.app` is). Pure Rust type re-exports — NO schemars/aide surface, so the E8
golden is byte-identical (machine-verified). ziee's lib is an app crate consumed
only by ziee-desktop; widening its surface is inert.

## Decision 6 — `postgresql_embedded` feature parity + the SDK `.cargo/config.toml`

Adding `postgresql_embedded` to `ziee-framework` risks (a) flipping default
features in the unified `src-app/Cargo.lock` and (b) the standalone SDK build
downloading a DIFFERENT default PG version (no `POSTGRESQL_VERSION` env).

**Resolution:** declare `postgresql_embedded = { version = "0.20.0",
default-features = false, features = ["bundled","theseus","rustls"] }` — byte-
matching the ziee server's `[workspace.dependencies]` decl, so feature-
unification adds nothing new for the server binary (`cargo check -p ziee` +
`-p ziee-desktop` exit 0 confirm). And add an SDK-standalone-only
`sdk/.cargo/config.toml` pinning `POSTGRESQL_VERSION="=18.3.0"` so
`cd sdk && cargo check --workspace` resolves ziee's exact PG version (fingerprint-
identical → shared-target reuse). Cargo config is taken from the INVOCATION root,
so this file is inert when ziee consumes the SDK by path — it cannot perturb
ziee's build.
