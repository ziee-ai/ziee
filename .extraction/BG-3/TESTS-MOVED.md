# Chunk BG-3 — TESTS-MOVED

BG-3 is an equivalence-preserving move + de-globalization — no new behaviour, so
no NEW tests. Existing tests move with their code / are threaded through the new
signatures.

## Moved WITH the code into `ziee-framework` (now `cargo test -p ziee-framework embedded_pg::`)

### `embedded_pg.rs` — 2 unit tests (verbatim; version const → literal)
- `stop_is_noop_when_no_postmaster_pid` — no `data/postmaster.pid` ⇒ clean no-op
  (never shells out to pg_ctl).
- `stop_returns_ok_when_pg_ctl_missing` — stale `postmaster.pid` + absent
  versioned `pg_ctl` binary ⇒ warn + Ok (exercises the cross-platform
  `<dir>/<version>/bin/pg_ctl[.exe]` path construction).

The only edit: the `POSTGRES_VERSION` const (an `env!("ZIEE_POSTGRES_VERSION")`
absent from the SDK workspace) is replaced by a `TEST_PG_VERSION = "18.3.0"`
literal — the tests never launch Postgres, so the version only feeds the path
construction. **Verified: `cargo test -p ziee-framework embedded_pg::` — 2
passed, 0 failed.**

## Threaded through the new signatures (ziee side — arg edits only, E4-clean)

### `desktop/tauri/tests/auth_tests.rs` — 4 tests (assertions unchanged)
`ensure_desktop_admin_creates_admin_on_first_run`,
`ensure_desktop_admin_is_idempotent`,
`mint_admin_login_returns_valid_jwt_for_bootstrapped_admin`,
`mint_admin_login_registers_whitelisted_jti`,
`mint_admin_login_errors_when_admin_missing` — each call site threads the
already-in-scope `shared_pool()` into `ensure_desktop_admin(pool)` /
`mint_admin_login(pool, &jwt)`. The behavioural assertions (admin created with
`is_admin`, idempotent single-admin, jti whitelisted + active, the "Admin not
found" error-prefix contract) are byte-unchanged. These are `#[ignore]`-free but
need Postgres :54321 — the RUN is the orchestrator's post-merge step (not part of
the BG-3 static gate; the golden + cargo-check are).

## No new integration/E2E

The desktop boot path change is verified statically here (cargo check + golden +
the moved unit tests). Its runtime proof is the **desktop permanent-session +
`auto_login` + 2-IPC-command E2E**, which cannot run in this Bash-tool harness —
it is the orchestrator's post-merge / D-full verification (the same boundary the
Chunk D STOP_REPORT named for the auth-carrying boot path). No behavioural
assertion was weakened to make a suite green (no `#[ignore]`/`.skip` added — E3).
