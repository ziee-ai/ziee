# TEST_RESULTS — logout session invalidation

Full logs (P4): `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/`
— `logout-session-int-serial.log` (authoritative integration run), `logout-session-int-2.log`
(docker-enabled parallel run), `logout-session-e2e.log`, `ui-check.log`, `check-all.log`.

## Unit — Rust (`cargo test --lib -p ziee auth::`)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS

## Integration — Rust

Run: `cargo test --test integration_tests -- --test-threads=1 auth:: sync::delivery mcp::builtin`
(under `sg docker -c` — the LDAP/OAuth suites need testcontainers).

- **TEST-4**: PASS — `test_logout_revokes_the_access_token`
- **TEST-5**: PASS — `test_logout_revokes_access_on_bare_jwtauth_routes`
- **TEST-6**: PASS — `test_logout_revokes_access_on_permission_gated_routes`
- **TEST-7**: PASS — `test_logout_then_immediate_relogin_yields_a_working_token`
- **TEST-8**: PASS — `test_logout_is_atomic_bump_rolls_back_if_revoke_fails`
- **TEST-9**: PASS — `test_refresh_racing_a_logout_does_not_outlive_it`
- **TEST-10**: PASS — `test_logout_leaves_other_users_sessions_alone`
- **TEST-11**: PASS — `test_pre_migration_ver_less_token_still_authenticates`
- **TEST-12**: PASS — `sync::delivery_test::logout_signals_the_users_other_tabs_but_not_the_origin_tab`
- **TEST-13**: PASS — `auth::test_auth_logout` (the comment that documented the bug is gone)
- **TEST-22**: PASS — `mcp::builtin_test_connection_test::builtin_probe_still_works_after_a_logout_bumped_the_token_epoch`

## Unit — frontend (vitest, jsdom)

Run: `npx vitest run src/modules/auth/Auth.store.test.ts` → 5 passed.

- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-18**: PASS

**Negative control (B7 — a green test proves nothing until it can go red):** reverting
`tearDownSession` to the pre-fix wipe makes **TEST-14 + TEST-16 FAIL** with
`AssertionError: expected [ 'users::read', 'users::edit', '*' ] to deeply equal []` — i.e. they
genuinely catch the admin's permissions surviving logout. Restored → 5/5 green.

## E2E — Playwright (`--workers=1`, under `sg docker -c`, ports 19000/19100) — **6 passed (2.8m)**

- **TEST-19**: PASS — `logout clears the persisted token`
- **TEST-20**: PASS — `logging out in one tab tears down the other tab without a reload` (THE reported cross-tab symptom)
- **TEST-21**: PASS — `a tab with no sync stream is still dead after another tab logs out` (the server-side backstop, proving the SSE signal is an optimisation not the boundary)
- Pre-existing specs also green (no regression from the new `Session` publish): `logging out via the profile dropdown returns to the login form`, `granting users::read … without reload`, `revoking permission … without reload`.

## Frontend gate

- `npm run check (ui): PASS` — all 18 sub-gates (tsc, lint:guardrails, lint:colors,
  lint:settings-field, check:kit-manifest, check:testid-registry, check:design-spec,
  check:gallery-coverage, check:state-matrix, check:overlay-registry, check:override-registry,
  check:gallery-seed-registry, …).
- `gate:ui (ui): N/A — no UI surface` — the diff adds no page/component/render state (the only
  frontend change is behavioral, inside `Auth.store.ts`), so there is no gallery surface for the
  runtime-health/visual pass to cover. `check:state-matrix` + `check:gallery-coverage` (inside
  `npm run check`, above) confirm no new conditional render state was introduced. Live browser
  verification was done instead, on a real build, with screenshots (see the STATUS file).

## Live verification (beyond the suites)

Re-ran the ORIGINAL repro on a real build at `:8091`. Every endpoint that returned **200** with a
post-logout token now returns **401 `SESSION_REVOKED`** (`/auth/me`, `/api/conversations`,
`/api/users`, `/api/onboarding/progress`, `/api/hub/installed`); logout → re-login in the same second
still yields a working token; the non-admin no longer sees the admin's conversations
(`showsPayroll: true → false`); tab 2 tears down to the login screen on its own.

## Known non-blocking environment notes (proven pre-existing, NOT caused by this diff)

- `auth::test_ensure_unique_username_collision_suffix_and_defaults` fails under `--test-threads=4`
  but **passes in isolation** and in the `--test-threads=1` run. It initializes the **global
  in-process `Repos` singleton** against its own per-test DB (`#[serial_test::serial(repos)]`), so a
  parallel test that already initialized `Repos` against a since-dropped DB makes it 500. This diff
  touches no part of that path. CLAUDE.md documents this class of load-related flake.
- `npx vitest run` (whole suite) fails to transform `SplitView.store.test.ts` — it is authored
  against `node:test` **on `origin/khoi` already**, and `vitest.config.ts`'s exclude list names only
  `MessageViewState.store.test.ts`. My diff touches neither file; all 40 tests pass. Not fixed here
  per rule B3 (never edit shared test config to route around your feature).
