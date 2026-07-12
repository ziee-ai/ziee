# PLAN — config-as-code desired-state reconciler

One declarative, env-templated file the ziee container reconciles into its DB on boot, so a fresh
deploy (TeamCity / `docker compose up`) comes up fully configured with NO manual UI setup.
Deliberately minimal: no new `Config` section, no config-template/envsubst edits, no MCP-repository
change, no test-harness change, no product UI code change.

Mechanism: `ZIEE_DESIRED_STATE_FILE` (env; defaulted in the Dockerfile to
`/etc/ziee/desired-state.yaml`) → `main.rs` calls `desired_state::reconcile(&pool)` AFTER
migrations + secret-key init, BEFORE serving. Unset env / missing file → silent no-op (dev,
desktop and every existing test unchanged). `${VAR}` placeholders resolve from process env
(secrets never inline, never logged). Every entry is existence-checked against a stable key before
writing → idempotent. Per-entry `mode: ensure | enforce`.

## Items

- **ITEM-1**: New module `src-app/server/src/modules/desired_state/mod.rs` — YAML types, `${VAR}`
  templating, `reconcile(pool)` orchestration. Soft-fails per entry (log + skip), never crashes boot.
- **ITEM-2**: MCP system servers (RCPA / DSCC / biognosia) created via the existing
  `Repos.mcp.create_system_server(CreateMcpServerRequest)` (pool-only; no JWT), guarded by a
  `name + is_system` existence check (there is NO unique index on `mcp_servers.name`, so an
  unguarded second call silently duplicates). URLs from `${RCPA_MCP_URL}` / `${DSCC_MCP_URL}` /
  `${BIOGNOSIA_MCP_URL}`; supports_sampling false/false/true; timeout_seconds 300 for rcpa+dscc;
  enabled true. `enforce` mode re-syncs url/display/enabled/timeout/supports_sampling on later boots.
- **ITEM-3**: Each declared server is assigned to the groups named in the file (`[Users]`) via the
  already-idempotent `mcp::repository::assign_server_to_group` — without this, non-admin users
  cannot use the servers.
- **ITEM-4**: Admin ensure — if `!Repos.user.has_admin()` → `Repos.app.create_admin_user("admin",
  "admin@tinnguyen-lab.com", hash(${ZIEE_ADMIN_PASSWORD}))`. If an admin already exists → no-op;
  the password is NEVER reset on a later boot. Unset/blank password env → skip + warn.
- **ITEM-5**: Normal (non-admin) user seeding via
  `Repos.auth.create_local_user_with_default_group`, skipped when the username or email already
  exists. Root admins bypass ALL permission checks (`users.is_admin`), so this account is what
  actually exercises the reduced-permission UI.
- **ITEM-6**: Group-permission reconcile — declarative `remove:` / `add:` lists (an entry ending
  `::*` matches by `::` hierarchy prefix), written with `Repos.group.update` (the REST route
  refuses permission edits on system groups). Shipped file removes `assistants::*`, `hub::*`,
  `projects::*` from the default `Users` group (mode `enforce`).
- **ITEM-7**: `config/desired-state.yaml` — the committed, env-templated file (3 MCP servers +
  admin + seeded user + the Users trim). Secrets appear only as `${...}` placeholders.
- **ITEM-8**: Boot wiring in `src-app/server/src/main.rs` (after `core::secrets::init_storage_key`,
  before the router is built) + `modules/mod.rs` registration. Amended (DRIFT-1.1): the same
  one-line call also goes in `src-app/server/src/lib.rs::setup_server` (the desktop/embedded boot
  path) — otherwise the lib never calls the module and every item in it is dead code. Still a no-op
  for desktop, which never sets `ZIEE_DESIRED_STATE_FILE`.
- **ITEM-9**: Migration 157 — `DELETE FROM mcp_servers WHERE is_system AND name IN
  ('filesystem','browser','git')`. These are the unused stdio rows seeded by migration 7
  (`browser`'s display_name is "Browser Automation"). `fetch` stays (enabled + assigned to the
  default group); `files` (`files.ziee.internal`) is a DIFFERENT, load-bearing built-in and is
  untouched. Migration 7 is NOT edited (sqlx checksums applied migrations — an in-place edit
  hard-fails every existing deployment).
- **ITEM-10**: Fix the tests that hard-depend on the deleted rows. Amended (DRIFT-1.4) — the real
  dependency set is: `server/tests/mcp/mod.rs` (a `>= 4` count assert, two `filesystem` lookups, a
  stale comment, AND a group-cascade test that needs a system server which is NOT already
  group-assigned — `fetch` ships assigned, so that test gets its own `cascade_target` fixture row);
  and the e2e specs, where `filesystem` was *the disabled* system server across five admin tests
  (toggle / search / filter-enabled / filter-disabled / list) — after the deletion no seeded
  DISABLED system server exists, so those specs seed a `Disabled Fixture` server via the admin API
  in `beforeEach` (the e2e harness gives each test its own database). The chat chip-row spec
  retargets onto `Web Fetch`, which is still `is_built_in`.
- **ITEM-11**: Docker + docs — `docker/web/Dockerfile` COPY + `ENV ZIEE_DESIRED_STATE_FILE`;
  `docker-compose.yml` passes `RCPA_MCP_URL` / `DSCC_MCP_URL` / `BIOGNOSIA_MCP_URL` /
  `ZIEE_ADMIN_PASSWORD` / `ZIEE_DEFAULT_USER_PASSWORD` through with empty defaults; a
  `docker/web/README.md` section documenting the env params, the mount/override, and the
  ensure-vs-enforce contract.

## Files to touch

New:
- `src-app/server/src/modules/desired_state/mod.rs`
- `config/desired-state.yaml`
- `src-app/server/migrations/00000000000157_remove_unused_builtin_mcp_servers.sql`
- `src-app/server/tests/desired_state/mod.rs`

Edit:
- `src-app/server/src/main.rs` (call the reconciler)
- `src-app/server/src/lib.rs` (same call on the desktop/embedded boot path — DRIFT-1.1)
- `src-app/server/src/modules/mod.rs` (register the module)
- `src-app/server/tests/integration_tests.rs` (`mod desired_state;`)
- `src-app/server/tests/mcp/mod.rs` (retarget the 3 asserts at `fetch`)
- `src-app/ui/tests/e2e/mcp/mcp-admin-servers.spec.ts` (disabled-server fixture),
  `src-app/ui/tests/e2e/chat/mcp-chip-row-persistence.spec.ts` ("Filesystem Access" → "Web Fetch")
- `src-app/ui/tests/e2e/permissions/desired-state-restricted-user.spec.ts` (new, TEST-14)
- `docker/web/Dockerfile`, `docker/web/README.md`, `docker-compose.yml`

## Patterns to follow

- **Config → DB, once, DB authoritative after** → `modules/auth/session_settings.rs::seed_from_config_once`.
- **Boot-time user creation** → `desktop/tauri/src/modules/auth/bootstrap.rs::ensure_desktop_admin`
  (`Repos.user.has_admin()` → `Repos.app.create_admin_user(...)`, bcrypt via `auth::password::hash_password`).
- **System-MCP creation from non-HTTP code** → `modules/hub/handlers.rs:1331`
  (`Repos.mcp.create_system_server*`); group assignment → `mcp/repository.rs::assign_server_to_group`
  (already `ON CONFLICT DO NOTHING`).
- **Group-permission removal** → `migrations/00000000000037_remove_hub_models_from_users_group.sql`;
  boot-time write → `Repos.group.get_by_name` + `Repos.group.update` (the REST handler refuses
  system-group permission edits — `user/handlers/groups.rs:176-191`).
- **Integration test shape** → `tests/bio_mcp/mod.rs:20-52` (mutating the seeded Users group),
  `tests/mcp/permission_revocation_test.rs` (grant→revoke→403), `TestServerOptions::extra_env`.
- **Restricted-user e2e** → `ui/tests/e2e/permissions/` fixtures (`create_user_with_only_permissions`
  analogue on the FE side).

## UI-surface checklist

**Not applicable — this feature adds NO new UI surface.** No page, drawer, card, panel or component
is added or restyled; the only frontend files touched are two existing e2e specs whose fixture
server row (`filesystem`) is being deleted, retargeted at the surviving `fetch` row. The
user-visible EFFECT (Projects / Hub / Assistants absent for a non-admin user) is a change to
existing, already-designed gating, asserted by a `[negative-perm]` e2e spec.
