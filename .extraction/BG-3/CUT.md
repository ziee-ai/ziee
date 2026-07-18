# Chunk BG-3 — de-globalize the desktop consumer boot path behind `ServerBoot` + relocate embedded-PG (CUT manifest)

BG-3 is the **prerequisite for D-full** (the live Tauri-shell move). It is
**two moves + one de-globalization**, all equivalence-preserving (E8 golden
byte-identical on BOTH surfaces — machine-verified, then restored):

1. **Relocate the generic embedded/external Postgres bootstrap** out of ziee's
   `server/core/database/mod.rs` into **`ziee-framework`'s `embedded_pg`**,
   parameterized over (a) the app's merged `&'static Migrator`, (b) the Postgres
   binary version string, and (c) two pgvector install/smoke-test hooks. Runtime
   code, **no `query!`** → `ziee-framework` stays **build-DB-free**.
2. **Implement the harness `ServerBoot` seam app-side** — ziee-desktop's
   `ZieeServerBoot` wraps `ziee::start_server_with_routes` + the desktop route
   re-layering and returns the harness's `BootHandle{addr,pool,jwt}`. The harness
   names ONLY the trait + `BootHandle` (verified: zero code `ziee::` in
   `sdk/desktop/harness/src`); ziee provides the concrete impl.
3. **Thread the desktop-consumer globals** — the live `start_backend_server`
   routes through `ZieeServerBoot::boot()` and threads the returned handle's
   `pool`/`jwt` into every post-boot consumer (migrations, ensure-owner,
   memory-default, the `auto_login` command) instead of re-reaching
   `ziee::Repos.{pool,user,app}` / a raw closure JWT.

## Files — SDK submodule (`sdk/`)

### NEW (2)
- `crates/ziee-framework/src/embedded_pg.rs` — the generic embedded/external PG
  lifecycle moved verbatim (behaviour-preserving) from ziee's
  `core/database/mod.rs`: `stop_existing_postgres_instance`,
  `initialize_database` (retry + `DATABASE_POOL` OnceCell), the embedded
  setup/start branch, `connect_with_retry`, `get_database_pool`,
  `cleanup_database`, the panic/Drop cleanup handlers + `POSTGRESQL_INSTANCE` /
  `_CLEANUP` statics. Parameterized (see TRANSFORMS). Its 2 in-source unit tests
  (`stop_is_noop_*`, `stop_returns_ok_*`) move with it.
- `.cargo/config.toml` — SDK-standalone-only `[env] POSTGRESQL_VERSION=` so the
  new `postgresql_embedded` dep resolves to ziee's exact PG version in
  `cd sdk && cargo …`. NOT used when ziee consumes the SDK by path (cargo config
  is taken from the ziee invocation root), so it cannot perturb ziee's build.

### MODIFIED (3)
- `crates/ziee-framework/src/lib.rs` — `pub mod embedded_pg;`.
- `crates/ziee-framework/Cargo.toml` — add `postgresql_embedded` (mirrors the
  ziee server catalog: `default-features=false` + `bundled,theseus,rustls`);
  extend `sqlx` (`+ runtime-tokio-rustls, migrate`) and `tokio`
  (`+ rt-multi-thread, time`); add `tempfile` dev-dep for the moved tests.
- `Cargo.lock` — regenerated (postgresql_embedded dep tree pulled into the
  standalone SDK workspace lock).

## Files — ziee app side (`src-app/`)

### MODIFIED (7)
- `server/src/core/database/mod.rs` — 592→lines shrunk to a thin orchestration
  shim: keeps the schema-/app-bound `MERGED_MIGRATOR` (with
  `set_ignore_missing(true)`) + the two pgvector hooks + the `pgvector_install`
  submodule + the `POSTGRES_VERSION` const; `initialize_database` /
  `get_database_pool` / `cleanup_database` delegate to
  `ziee_framework::embedded_pg` with byte-identical public signatures (every
  consumer — `main.rs`, `lib.rs`, `file::geometry_backfill` — unchanged).
- `server/src/lib.rs` — re-export `AppRepository` + `UserRepository` so the
  desktop consumer builds them from a threaded pool (no wire impact — Rust type
  re-exports, golden byte-identical).
- `desktop/tauri/Cargo.toml` — add the `ziee-desktop-harness` path dep.
- `desktop/tauri/src/modules/backend/mod.rs` — NEW `mod server_boot`; rewire
  `start_backend_server` to route through `ZieeServerBoot::boot()`; add
  `SERVER_POOL` stash + `get_server_pool()`; thread `handle.{pool,jwt}` into the
  post-boot steps; `run_desktop_migrations` + `enable_memory_admin_default` now
  take `&PgPool`.
- `desktop/tauri/src/modules/backend/server_boot.rs` — NEW `ZieeServerBoot`, the
  concrete `ServerBoot` impl (`boot()` → `{addr,pool,jwt}`; `shutdown()` →
  `cleanup_server`).
- `desktop/tauri/src/modules/auth/commands.rs` — `mint_admin_login(pool, jwt)`
  threaded (`UserRepository::new(pool)` = the same repo `Repos.user` builds);
  `auto_login` sources pool from the `SERVER_POOL` stash.
- `desktop/tauri/src/modules/auth/bootstrap.rs` — `ensure_desktop_admin(pool)`
  threaded (`UserRepository::new(pool)` + `AppRepository::new(pool)`; create
  stays app-side per BA).

### MODIFIED — tests (1)
- `desktop/tauri/tests/auth_tests.rs` — arg edits only (thread the already-in-
  scope `pool` into `ensure_desktop_admin` / `mint_admin_login`); **zero
  assertion-body edits** (E4).

`Cargo.lock` (`src-app/`) — regenerated (new edge ziee-desktop → harness).

## What STAYS app-side (per Chunk D + BA — not moved)
`pgvector_install` (build.rs `OUT_DIR` include), the `MERGED_MIGRATOR` +
`POSTGRES_VERSION` const, `create_desktop_modules` + desktop-only modules, CORS
allowlist / feature overrides / branding, `AppRepository::create_admin_user`
(owner-create domain CRUD), `BACKEND_CONFIG`/`BACKEND_STATE` (app Tauri
plumbing). The live Tauri shell MOVE (`run`/`run_headless`, the 2 IPC commands,
`create_main_window`) is **D-full** — now unblocked by this seam.
