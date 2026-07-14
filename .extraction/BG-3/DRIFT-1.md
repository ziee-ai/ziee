# Chunk BG-3 — DRIFT scan (round 1)

Drift = any place the move / de-globalization could diverge from pre-move
behaviour or surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. `embedded_pg.rs` moved with the generic
  lifecycle byte-verbatim; only the 3 declared seams (migrator / pg_ctl version /
  pgvector hooks) + the config-type differ (TRANSFORMS D1/D2). Settings
  population, retry counts + backoff formula, the OnceCells, the SELECT-1
  re-check, and every log string are identical.

- **DRIFT-1.2** — verdict: none. pgvector interleave. The `after_setup` +
  `smoke_test` hooks are called at the exact points the inline code ran
  (after `setup()` / after `start()`), and ziee's hook bodies reproduce
  `has_real_artifacts`/`install_into` + `CREATE EXTENSION IF NOT EXISTS vector` +
  `mark_available` with the same WARN strings. A stubbed pgvector build still
  fail-softs identically (the memory module reads `pgvector_install::is_available`).

- **DRIFT-1.3** — verdict: none. The migrator. `MERGED_MIGRATOR` is the same
  `migrations-merged` set with `set_ignore_missing(true)`; a `&'static Migrator`
  is passed and `migrator.run(&pool)` replaces the inline
  `sqlx::migrate!(...).set_ignore_missing(true).run(&pool)`. Same migrations,
  same ignore-missing → identical `_sqlx_migrations` outcome. DB-schema-snapshot
  parity is inherited (no migration file changed).

- **DRIFT-1.4** — verdict: none. Cleanup statics. `DATABASE_POOL` /
  `POSTGRESQL_INSTANCE` / `CLEANUP_REGISTERED` / `_CLEANUP` move as ONE unit into
  the framework, so `cleanup_database` still reads both cells from one crate.
  One `ziee-framework` in the graph ⇒ one instance of each ⇒ same
  panic-hook/Drop behaviour. `ziee::cleanup_server` → `core::database::cleanup_database`
  → framework `cleanup_database` (unchanged path).

- **DRIFT-1.5** — verdict: none. The desktop boot closure. `ZieeServerBoot::boot`
  contains the identical route-builder body (repos init, merge + CORS +
  Extension(jwt), dev/prod fallback) and post-boot orchestration is byte-order-
  identical in `start_backend_server`. The only delta (jwt captured out into the
  BootHandle vs written to the module static inside the closure) is reconciled in
  DRIFT-1.6.

- **DRIFT-1.6** — verdict: none (observable). `JWT_SERVICE`/`SERVER_POOL` are set
  a few statements later (after boot returns) than the old in-closure
  `JWT_SERVICE.set`, but both precede `create_main_window` by the full
  migrations+bootstrap sequence, and `auto_login` is frontend-invoked only after
  the window loads. No caller can observe the reorder. The stashes hold the same
  jwt/pool the globals held.

- **DRIFT-1.7** — verdict: none. Auth consumers. `mint_admin_login` /
  `ensure_desktop_admin` use `UserRepository::new(pool)` / `AppRepository::new(pool)`
  — the SAME repository types `Repos.{user,app}` are, built from the SAME pool.
  `get_by_username`/`has_admin`/`create_admin_user`/`mint_session_tokens` are
  unchanged; error strings preserved verbatim. The jti-whitelist + owner-`*`
  posture is untouched.

- **DRIFT-1.8** — verdict: none. Wire surface. The two new `ziee` re-exports
  (`AppRepository`/`UserRepository`) carry no `JsonSchema`/aide surface;
  `embedded_pg` + `server_boot` expose no handler/DTO. E8 golden byte-identical
  (types.ts, both surfaces) + canonical (openapi.json, both surfaces),
  machine-verified then restored via `git checkout`.

- **DRIFT-1.9** — verdict: none. Build/feature. `postgresql_embedded` matches the
  server's base decl; the extra `sqlx`/`tokio` features are already server-
  enabled, so no new default features unify onto the server binary.
  `cargo check -p ziee`, `-p ziee-desktop`, and `cd sdk && cargo check --workspace`
  all exit 0. `ziee-framework` stays build-DB-free.

- **DRIFT-1.10** — verdict: none. Tests. The 2 embedded-PG `stop_*` unit tests
  moved WITH the file and pass under the SDK workspace (version literal replaces
  the `POSTGRES_VERSION` const). The desktop `auth_tests.rs` needed arg-only edits
  (thread the in-scope pool); assertions unchanged.

**Unresolved drifts: 0**
