# TESTS — enumerated up front

No new permission is introduced (the reconciler reuses existing auth-provider
storage) and no frontend workspace is touched → no `[negative-perm]` e2e and no
`tier: e2e` test are required (A9/A10 and the UI-e2e gate do not apply).

## Unit (inline `#[cfg(test)] mod tests` in `modules/desired_state/mod.rs`)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: a document with an `auth_providers:` block (name/client_id/client_secret/enabled/mode) parses via `serde_norway::from_str`, and an `auth_providers` entry with an unknown field is rejected (`deny_unknown_fields`).
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: `plan_auth_provider` returns `Skip` when `GOOGLE_CLIENT_ID` and/or `GOOGLE_CLIENT_SECRET` resolve empty/unset (via `map_lookup`), and never returns `Stamp` on a partial (only one of the two set).
- **TEST-3** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: `plan_auth_provider` returns `Skip` (InlineSecret) when `client_secret` is an inline literal (e.g. `"hunter2"`) rather than a single `${VAR}` placeholder.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: on both vars set, `plan_auth_provider` returns `Stamp{enabled:true, config}` whose config sets `client_id` + plaintext `client_secret` to the resolved values AND preserves the seeded `issuer_url`/`scopes`/`attribute_mapping`/`display_name` unchanged.
- **TEST-8** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: the shipped `config/desired-state.yaml` parses, contains an `auth_providers` entry named `google` with `mode: enforce` + `enabled: true`, and its `client_secret` is a single `${...}` placeholder (no inline secret). (Extends the existing `shipped_desired_state_file_is_valid` / TEST-17.)
- **TEST-9** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: the shipped `docker-compose.deploy.yml` passes BOTH `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET` through to `ziee-web` with an empty (`:-`) default — a regression guard so a future compose edit can't silently drop the deploy wiring. (The `docker/web/README.md` docs are the doc half of the same ITEM-5 deploy-wiring unit.)

## Integration (`src-app/server/tests/desired_state/mod.rs`, real boot path)

- **TEST-5** (tier: integration) [covers: ITEM-3, ITEM-4] file: `src-app/server/tests/desired_state/mod.rs` — asserts: with `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` set (dummy values) and the shipped-shape `google` entry in the manifest, after boot the `google` row has `enabled=true`, `config->>'client_id'` = the dummy id, `config->>'client_secret'` is BLANK, and `pgp_sym_decrypt(client_secret_encrypted, <harness storage_key>)` = the dummy secret (mirrors `admin_providers_test.rs:165-215`).
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/desired_state/mod.rs` — asserts: with `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` UNSET, after boot the `google` row is still `enabled=false` with empty `config->>'client_id'` and NULL `client_secret_encrypted` (the entry was skipped — Google stays disabled).
- **TEST-7** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/desired_state/mod.rs` — asserts: a second boot (`reboot()`, same DB, storage_key present) is idempotent — still exactly one `google` row, `enabled=true`, `config->>'client_id'` stable, and the decrypted secret still matches (no dupes, no plaintext leak).

## Item → test coverage

- ITEM-1 → TEST-1
- ITEM-2 → TEST-2, TEST-3, TEST-4
- ITEM-3 → TEST-5, TEST-6, TEST-7
- ITEM-4 → TEST-5, TEST-8
- ITEM-5 → TEST-9
