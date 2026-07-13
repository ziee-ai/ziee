# INFRA_INTEGRATION — the two mandatory walks

## User-experience walk (how a real user encounters this)

- The "user" here is the **deploy operator** (TeamCity), not an end user. The
  operator sets `GOOGLE_CLIENT_ID` + `GOOGLE_CLIENT_SECRET` as TeamCity params on
  the deploy config; on the next deploy the container boots, reconciles, and the
  **login page shows "Sign in with Google"** with zero admin-UI steps.
- End-user flow (already existing, unchanged): click "Sign in with Google" →
  Google consent → `/api/auth/oauth/google/callback` → logged in.
- Failure the operator must be able to diagnose: a `redirect_uri_mismatch` from
  Google. Mitigated by DEPLOY.md + the config-as-code README both stating the
  exact redirect URI (`<public-origin>/api/auth/oauth/google/callback`) and the
  `X-Forwarded-Proto/Host` requirement. The reconcile log names which provider it
  configured/skipped (never the secret value) so the operator can confirm from
  container logs.
- Local-dev "user" (a developer): with the two vars unset (and/or the deploy flag
  off), Google is a no-op — their hand-configured provider is never touched.

## Infrastructure-integration walk (every subsystem this item touches)

- **desired_state reconciler** — runs under the existing advisory lock + gate
  (`ZIEE_APPLY_DESIRED_STATE`); the new branch is one more loop in
  `reconcile_entries`, after mcp_servers. No new lock, no ordering hazard (auth
  providers don't depend on users/groups/mcp).
- **auth/providers repository** — reuses `get_provider_by_name` + `update_provider`
  (the admin-CRUD encrypt path). No repo change → no new caller contract to audit.
  `update_provider(name:None)` leaves the name/provider_type intact.
- **core::secrets storage_key** — `init_storage_key` runs in `main.rs` BEFORE
  `reconcile` (verified: main.rs:218 init, main.rs:235 reconcile), so
  `prepare_config_for_write`/`encrypt_secret` see the key. In compat mode (no
  key) the secret stays plaintext in config — the SAME documented fallback as the
  admin CRUD path; not a regression this branch introduces.
- **OAuth login flow** — consumes `auth_providers.config` (issuer/scopes/mapping)
  + the decrypted `client_secret`; we preserve those seeded fields and only set
  client_id/client_secret/enabled, so the existing callback keeps working.
- **MCP boot health check** — unrelated to auth providers (it only probes MCP
  servers). But the analogous risk (a provider getting disabled out-of-band) is
  why google ships `enforce`: the admin health/test path (`enforce_on_update_transition`
  live-probe) or an admin toggle could disable it; the next deploy re-enables it.
- **Realtime sync** — the reconcile writes the row directly at boot before serving;
  no client is connected yet, so no `sync:` emit is needed (matches how the mcp
  reconcile writes without emitting). The admin provider CRUD path (which DOES emit)
  is unchanged.
- **Permissions / authz** — no new permission. The reconciler runs as a boot task
  (no JWT/user context); the runtime admin CRUD that gates provider edits
  (`auth::providers::manage`) is untouched. Nothing new to gate.
- **Migrations** — none added; relies on migration 47 (seed) + 125 (encrypted col)
  already present on base.
- **docker-compose.deploy.yml / README / DEPLOY.md** — env passthrough + docs; no
  runtime behavior beyond making the two vars available to the container.
