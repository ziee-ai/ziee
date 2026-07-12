# TESTS — config-as-code

Tiers mirror the codebase: unit = in-source `#[cfg(test)]`; integration =
`src-app/server/tests/desired_state/`; e2e = `src-app/ui/tests/e2e/`.

Integration tests drive the reconciler through the REAL boot path: they write a desired-state YAML
to a temp file and pass `ZIEE_DESIRED_STATE_FILE` + the `${VAR}` values via
`TestServerOptions::extra_env`, so the spawned `ziee` binary reconciles at boot exactly as the
container does. No shared-harness edit (B3).

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: `${VAR}` placeholders resolve from env; an unresolved `${VAR}` yields the "unresolved" error (which the caller turns into skip-entry), NOT a panic; `$$` / bare `$` / a literal `${}` are left intact.
- **TEST-2** (tier: unit) [covers: ITEM-1, ITEM-7] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: a `password` field whose value is NOT a `${...}` placeholder is REJECTED (inline-secret guard), and a `${...}` one is accepted.
- **TEST-3** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: the permission set-ops — `remove: ["hub::*"]` strips every `hub::…` entry by `::` hierarchy prefix (and `hub::models::read`), an exact string removes only itself, `add` is idempotent (no duplicate), and a no-op change is detected as such (no write).
- **TEST-4** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/desired_state/mod.rs` — asserts: YAML parse — a full valid document deserializes (servers/admin/users/groups); an unknown `mode:` value errors; a missing required field (e.g. server `url`) errors; an empty document is a legal no-op.

## Integration

- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3, ITEM-8] file: `src-app/server/tests/desired_state/mod.rs` — asserts: on a fresh DB the 3 declared system servers (rcpa/dscc/biognosia) exist EXACTLY ONCE with the right `url` (from env), `supports_sampling` (false/false/true), `timeout_seconds` (300/300/default), `enabled=true`, `is_system=true`, `user_id IS NULL` — and each is assigned to the `Users` group in `user_group_mcp_servers`.
- **TEST-6** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/desired_state/mod.rs` — asserts: **idempotency** — running the reconciler a SECOND time against the same DB creates NO duplicate rows (count still 1 per name) and NO duplicate group assignments; an `ensure`-mode server whose row was edited in the DB (e.g. `enabled` flipped off) is NOT clobbered, while an `enforce`-mode server IS re-synced back to the file's values.
- **TEST-7** (tier: integration) [covers: ITEM-1, ITEM-7] file: `src-app/server/tests/desired_state/mod.rs` — asserts: env-secret resolution — a server whose `${…_MCP_URL}` env var is UNSET is skipped (no row created) while the other entries still apply and the server boots healthy; a file with an INLINE password literal is rejected (no user created) and boot still succeeds.
- **TEST-8** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/desired_state/mod.rs` — asserts: admin ensure — on a fresh DB the `admin` user is created (`is_admin=true`, member of Administrators + Users) and can log in with `${ZIEE_ADMIN_PASSWORD}`; a SECOND reconcile with a DIFFERENT `ZIEE_ADMIN_PASSWORD` does **NOT** reset the password (login with the ORIGINAL still succeeds, login with the new one fails) and creates no second admin.
- **TEST-9** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/desired_state/mod.rs` — asserts: the declared normal user is created, is `is_admin=false`, is a member of the default `Users` group, can log in with `${ZIEE_DEFAULT_USER_PASSWORD}`; a second reconcile does not duplicate or re-hash it.
- **TEST-10** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/desired_state/mod.rs` — asserts: after reconcile the `Users` group's `permissions` array contains NO `assistants::*`, NO `hub::*`, NO `projects::*`, while `profile::read`, `chat::read`, `conversations::*`, `mcp_servers::read`, `user_llm_providers::read`, `files::read` are all still present (the KEEP set).
- **TEST-11** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/desired_state/mod.rs` — asserts: the BACKEND deny half (A9) — the seeded non-admin user gets **403** on `GET /api/assistants` and on a `hub::*`-gated route after the trim, while `GET /api/profile`-class and `GET /api/mcp/servers`-class calls still return 200.
- **TEST-12** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/desired_state/mod.rs` — asserts: migration 157 — after boot there is NO `is_system` row named `filesystem`, `browser`, or `git`; the `fetch` row still exists, is `enabled`, and is still assigned to the default group; the `files` built-in (`files.ziee.internal`) row is untouched.
- **TEST-13** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/mcp/mod.rs` — asserts: the retargeted existing MCP suite still passes against the post-deletion seed set (the system-server list, the editable-built-in update path, and the group-assignment test now key off `fetch` instead of the deleted `filesystem`).

## E2E

- **TEST-14** (tier: e2e) [negative-perm] [covers: ITEM-6, ITEM-5] file: `src-app/ui/tests/e2e/permissions/desired-state-restricted-user.spec.ts` — asserts: a user LACKING `projects::*` / `hub::*` / `assistants::*` (the reconciled default-group user) sees NO Projects nav entry, NO Hub nav entry, and NO "Assistants" tab in Settings — and deep-linking `/projects`, `/hub`, `/settings/assistants` renders the not-authorized result, not the feature — while General, Profile, LLM providers and MCP servers ARE present. Walks all four gating layers (slot → route → `<Can>` → `usePermission`).
- **TEST-15** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/07-mcp/mcp-admin-servers.spec.ts` — asserts: the admin MCP-servers specs (enable-toggle, search, built-in badge, edit-configurable-built-in) still pass, retargeted from the deleted "Filesystem Access" card onto the surviving "Web Fetch" card.
- **TEST-16** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/chat/mcp-chip-row-persistence.spec.ts` — asserts: the chat MCP chip-row persistence spec still passes with its fixture server retargeted from "Filesystem Access" to "Web Fetch".

## Coverage map (every ITEM → ≥1 TEST)

| ITEM | Tests |
|---|---|
| ITEM-1 (module/templating) | TEST-1, TEST-2, TEST-4, TEST-7 |
| ITEM-2 (MCP servers) | TEST-5, TEST-6, TEST-7 |
| ITEM-3 (group assignment) | TEST-5, TEST-6 |
| ITEM-4 (admin ensure) | TEST-8 |
| ITEM-5 (seeded user) | TEST-9, TEST-14 |
| ITEM-6 (group perms) | TEST-3, TEST-10, TEST-11, TEST-14 |
| ITEM-7 (desired-state.yaml) | TEST-2, TEST-7 |
| ITEM-8 (boot wiring) | TEST-5 (proves the real boot path ran) |
| ITEM-9 (migration 157) | TEST-12 |
| ITEM-10 (test retargets) | TEST-13, TEST-15, TEST-16 |
| ITEM-11 (docker + docs) | TEST-17 |

- **TEST-17** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/desired_state/mod.rs` — asserts: the SHIPPED `config/desired-state.yaml` (the exact file baked into the image) parses and validates — every secret is a `${...}` placeholder, every declared group/server key is well-formed — so a typo in the deploy file fails CI, not the deploy. (This is the machine-checkable half of ITEM-11; the Dockerfile/compose/README wiring itself is proven by the live container run in Phase 8's verification.)
