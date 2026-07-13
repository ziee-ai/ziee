# PLAN_AUDIT — plan vs. codebase

## Breakage risk

- The change is **purely additive**: a new `#[serde(default)]` field on
  `DesiredState`, two new functions, one dispatch line. No existing struct field,
  function signature, or call site changes → no existing caller breaks.
- `reconcile_entries` gains one loop over `desired.auth_providers`; the vec is
  `#[serde(default)]` so every existing manifest (and every test manifest that
  omits the key) parses to an empty vec → the branch is a no-op for them.
- `get_provider_by_name` / `update_provider` are already public and used by the
  admin CRUD handlers; calling them from the reconciler adds a second caller, not
  a modification. `update_provider(id, None, Some(enabled), Some(&config))` uses
  COALESCE + the `config_provided` CASE guard, so passing `name: None` leaves the
  name intact and only touches `enabled`, `config`, `client_secret_encrypted`.
- **Skip-safety**: if `GOOGLE_CLIENT_ID`/`SECRET` are unset, `plan_auth_provider`
  returns `Skip` before any DB write → the seeded `google` row is never touched,
  so a local dev's manually-configured provider is never clobbered. This is the
  same skip discipline `reconcile_mcp_server` uses for an unset URL.
- **Compat mode** (no `secrets.storage_key`): `prepare_config_for_write` keeps the
  secret plaintext in `config` and clears the encrypted column — identical to the
  admin CRUD path's documented fallback. Not a regression this branch introduces.

## Pattern conformance

- `AuthProviderEntry` mirrors `McpServerEntry` (`deny_unknown_fields`, `Mode`,
  `default_true`) — conforms.
- `reconcile_auth_provider`'s `(existing, mode)` match mirrors
  `reconcile_mcp_server` (ensure = leave fields; enforce = re-sync). One
  deliberate divergence: **absent row → skip, never create** (the mcp branch
  creates). Rationale: an auth provider requires a `provider_type` + a full config
  shape that only a seed migration supplies; the task says "only ADDS/UPDATES an
  existing row; never delete." Documented in DECISIONS (DEC-2).
- Secret resolution reuses `resolve_secret_with` exactly as `reconcile_user` does.
- Encryption reuses the repository helpers — the SAME path as `admin_update_provider`.
- Unit tests use the existing `map_lookup` injected-lookup style (no env mutation);
  integration tests mirror `tests/desired_state/mod.rs` + the encrypt-decrypt
  assertions of `tests/auth/admin_providers_test.rs`.

## Migration collisions

- **None.** This branch adds no migration. The `google` row (migration 47) and
  `client_secret_encrypted` column (migration 125) already exist on `khoi`.
  Highest migration on base is 157; untouched.

## OpenAPI regen

- **Not required.** No REST endpoint, request, or response type changes. The
  reconciler is a boot-time DB writer. `openapi.json` / `api-client/types.ts` in
  both `ui` and `desktop/ui` are untouched → no `just openapi-regen`.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new `#[serde(default)]` field + struct mirroring `McpServerEntry`; no existing manifest breaks (empty-vec default).
- **ITEM-2** — verdict: PASS — pure fn reusing `resolve_with`/`resolve_secret_with`; unit-testable with `map_lookup`, no DB, no env mutation.
- **ITEM-3** — verdict: PASS — reuses `get_provider_by_name`/`update_provider` (existing encrypt path); additive second caller; skip-if-unset preserves local dev. Absent-row-skip divergence from mcp is intentional (DEC-2).
- **ITEM-4** — verdict: PASS — additive top-level YAML key; placeholders only (no inline secret) so `shipped_desired_state_file_is_valid` (TEST-17) stays green; requires adding `auth_providers` to `DesiredState` (ITEM-1) to parse — sequenced correctly.
- **ITEM-5** — verdict: PASS — additive compose env passthrough (empty default, mirrors the `ZIEE_ADMIN_*` optional vars) + README docs; no behavior change when unset.
