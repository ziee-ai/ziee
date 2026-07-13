# PLAN — desired_state → env-driven Google OIDC enablement on deploy

## Context

On a deploy (`ZIEE_APPLY_DESIRED_STATE=1`), ziee's config-as-code reconciler
(`modules/desired_state/mod.rs`) already seeds/enforces MCP servers, the root
admin, and group permissions from the committed `config/desired-state.yaml` +
container env. The full Google OIDC flow already exists: migration 47 seeds a
`google` `auth_providers` row (`provider_type='oidc'`, `enabled=false`, config
prefilled with `issuer_url`/`scopes`/`attribute_mapping`/`display_name`); the
admin CRUD path encrypts `client_secret` at rest (migration 125:
`client_secret_encrypted` BYTEA, plaintext blanked in `config`). Today the only
way to turn Google on is a manual admin-UI edit.

This adds the one missing reconcile branch: on deploy, stamp
`client_id`/`client_secret` (from `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` env)
onto that seeded row and flip `enabled=true` — no admin step, no secret in the
repo. Mirrors the MCP-server seeding exactly (opt-in, default-OFF,
skip-if-env-unset, `enforce` re-asserts each boot), so local dev stays a no-op.

Not a UI change (backend + config + docs). No new permission, no new migration.

## Items

- **ITEM-1**: Add `AuthProviderEntry` struct + `#[serde(default)] auth_providers: Vec<AuthProviderEntry>` field to `DesiredState`, mirroring `McpServerEntry`. Fields: `name` (idempotency key), optional `client_id`, optional `client_secret` (single `${VAR}` only), `enabled` (default true), `mode`.
- **ITEM-2**: Add pure `plan_auth_provider(entry, existing_config, lookup) -> AuthPlan` (`Skip(reason)` | `Stamp{config, enabled}`). Resolve `client_id` via `resolve_with`, `client_secret` via `resolve_secret_with`; `None`/unset/empty → `Skip`; inline-literal secret → `Skip(InlineSecret)`. On success, clone `existing_config`, set `client_id`+`client_secret` (plaintext for the repo to encrypt), preserve `issuer_url`/`scopes`/`attribute_mapping`/`display_name`; return `Stamp{config, enabled}`.
- **ITEM-3**: Add `async fn reconcile_auth_provider(entry)` + one dispatch line in `reconcile_entries`. Look up via `providers::repository::get_provider_by_name(Repos.pool(), name)`; `None` → warn+skip (never create, parallels never-delete); `(Some, Ensure)` → debug no-op (fields untouched); `(Some, Enforce)` → `plan_auth_provider`; `Skip` → warn+return; `Stamp` → `update_provider(Repos.pool(), id, None, Some(enabled), Some(&config))` which routes through `prepare_config_for_write`/`encrypt_secret`+`storage_key()` (encrypt secret, blank plaintext, set enabled). Log env var NAMES only, never values. Idempotent (keyed UPDATE, no dupes).
- **ITEM-4**: Add the `auth_providers:` section (with the `google` entry: `enabled: true`, `client_id: ${GOOGLE_CLIENT_ID}`, `client_secret: ${GOOGLE_CLIENT_SECRET}`, `mode: enforce`) + a banner comment block to `config/desired-state.yaml` (env-only creds, skip-if-unset, pre-seeded by migration 47, redirect URI `<public-origin>/api/auth/oauth/google/callback`). No inline secret so TEST-17 stays green.
- **ITEM-5**: Deploy wiring — pass `GOOGLE_CLIENT_ID: "${GOOGLE_CLIENT_ID:-}"` + `GOOGLE_CLIENT_SECRET: "${GOOGLE_CLIENT_SECRET:-}"` through the `ziee-web` service in `docker-compose.deploy.yml` (empty default → clean skip), AND document the new `auth_providers` manifest block + the two env vars in `docker/web/README.md` config-as-code section (the two are one deploy-wiring unit; `DEPLOY.md` is already done on `khoi`).

## Files to touch

- `src-app/server/src/modules/desired_state/mod.rs` (schema + `plan_auth_provider` + `reconcile_auth_provider` + dispatch + inline unit tests)
- `src-app/server/tests/desired_state/mod.rs` (integration tests + `manifest()`/`env_for` google additions + `reboot()` storage_key)
- `config/desired-state.yaml`
- `docker-compose.deploy.yml`
- `docker/web/README.md`

## Patterns to follow

- **Schema + reconcile branch**: mirror `McpServerEntry` / `reconcile_mcp_server`
  in the SAME file (`modules/desired_state/mod.rs`) — `#[serde(deny_unknown_fields)]`,
  `Mode` enum, `default_true`, the `(existing, mode)` match, `tracing` log style
  (`desired_state:` prefix, warn on skip, info on reconcile, values never logged).
- **Secret resolution**: reuse `resolve_secret_with` exactly as `reconcile_user`
  does for `password` (single `${VAR}`, inline-literal rejected).
- **At-rest encryption**: reuse `providers::repository::{get_provider_by_name,
  update_provider}` (→ `prepare_config_for_write`/`encrypt_secret`/`storage_key()`)
  — the SAME path the admin CRUD `admin_update_provider` uses. Never hand-roll SQL.
- **Unit tests**: inline `#[cfg(test)] mod tests` with the `map_lookup` helper
  (no process-env mutation), mirroring the existing `resolve_*` tests.
- **Integration tests**: mirror `tests/desired_state/mod.rs` (real boot via
  `server_with`/`reboot`, `pool_of` DB assertions) and the encrypt-decrypt
  assertions in `tests/auth/admin_providers_test.rs:165-215`.
