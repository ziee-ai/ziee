# TEST_RESULTS

Backend-only diff (no `src-app/ui/**` or `src-app/desktop/ui/**` touched, per BASE.md)
→ only the backend chain applies; no `npm run check` / gate:ui / e2e required.
No new permission introduced → A9/A10 N/A.

## Unit (`cargo test --lib -p ziee desired_state::`) — 23 passed / 0 failed

- **TEST-1**: PASS  (`parses_an_auth_providers_block`)
- **TEST-2**: PASS  (`plan_auth_provider_skips_when_env_unset`)
- **TEST-3**: PASS  (`plan_auth_provider_rejects_an_inline_secret`)
- **TEST-4**: PASS  (`plan_auth_provider_stamps_and_preserves_seeded_fields`)
- **TEST-8**: PASS  (`shipped_desired_state_file_is_valid` — google entry + fixed names)
- **TEST-9**: PASS  (`shipped_deploy_compose_passes_google_env_through`)

## Integration (`cargo test --test integration_tests desired_state:: --test-threads=4`) — 15 passed / 0 failed

Log: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/dsap-int.log`
Ran the freshly-built `ziee` binary (harness spawns `src-app/target/debug/ziee` →
symlinked to the fresh CARGO_TARGET_DIR build; the google tests assert
new-code-only behavior, so a stale binary would have failed — no stale-binary trap).

- **TEST-5**: PASS  (`test_google_provider_configured_and_enabled_from_env` — enabled + client_id + decrypted secret + blanked plaintext)
- **TEST-6**: PASS  (`test_google_provider_skipped_when_env_unset` — disabled, blank, no blob)
- **TEST-7**: PASS  (`test_google_provider_reconcile_is_idempotent` — one row, re-enabled, secret stable)
- **TEST-10**: PASS (`test_google_provider_ensure_mode_leaves_row_untouched` — audit FIX-1)

## Live real-boot verification (task-required)

Booted the real `ziee` binary against real Postgres (external, :54321) with the
deploy switch on, `secrets.storage_key` set, and the shipped-shape google manifest.
Full logs: `scratchpad/liveboot-{set,unset}.log`.

- **Creds SET** (`GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` = dummy values):
  - log: `desired_state: auth provider configured + enabled from env provider=google enabled=true` → `reconcile complete`
  - DB row: `enabled=t`, `config->>'client_id'` = the dummy id, `config->>'client_secret'` = BLANK, `client_secret_encrypted` NOT NULL and `pgp_sym_decrypt(...)` = the dummy secret, `issuer_url` preserved.
- **Creds UNSET** (fresh DB, same manifest, no google env):
  - log: `WARN desired_state: auth provider skipped (set its client_id + client_secret env vars to enable it) provider=google reason=google: env var ${GOOGLE_CLIENT_ID} is unset or empty` → `reconcile complete`
  - DB row: `enabled=f`, `client_id` blank, `client_secret_encrypted` NULL — Google stays disabled; server boots normally (skip never crashes boot).

Both cases match the intended behavior exactly.
