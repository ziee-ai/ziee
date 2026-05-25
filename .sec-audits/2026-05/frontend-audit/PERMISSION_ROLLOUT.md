# Frontend permission-gating rollout — module checklist

**Branch:** `feat/frontend-permission-gating`
**Plan:** `~/.claude/plans/there-is-another-serene-coral.md`

Companion to the read-only audits in this folder (especially
`03-auth-user-permissions.md` and `07-hub-assistants-settings-misc.md`)
which enumerated the gaps. This doc tracks the rollout itself: one row
per gated surface, marked as it lands.

Status legend: `[x]` shipped · `[ ]` outstanding · `[-]` n/a (no
gating needed — public surface or already gated by backend with no UI
exposure).

---

## Foundation — DONE

- [x] `core/permissions/` helpers (`types`, `hasPermission`,
      `evaluatePermission`, `usePermission`, `Can`).
- [x] Slot type extensions: `SettingsPageSlot`, `SidebarNavItem`,
      `SidebarToolItem`, `HubTabSlot` (multi-verb).
- [x] Slot consumer enforcement: `SettingsPage` (menu filter + inline
      403), `LeftSidebar` (nav + tools filter).
- [x] Sandbox helper migration → `core/permissions`.

## Hub — DONE (worked example)

- [x] Sidebar entry gated by `anyOf` of the three submodule reads.
- [x] `HubPage`: visibleTabs filter, inline 403, per-tab `canRefresh`.
- [x] Submodule registrations: `permissions: { read, refresh }`.
- [x] Card actions: `McpServerHubCard` (`hub::mcp_servers::create`),
      `ModelHubCard` (`hub::models::download`), `AssistantHubCard`
      (`hub::assistants::create`).
- [-] Drawers: read-only display, no buttons to gate.

---

## Per-module rollout

### user — Users + Groups — DONE

Module: `src/modules/user/`. Backend perms: `users::*`, `groups::*`,
`profile::*`.

- [x] `module.tsx`: `settingsAdminPages[users]` → `users::read`,
      `settingsAdminPages[user-groups]` → `groups::read`.
- [x] `UsersSettings`: Create+ → `users::create`; Edit →
      `users::edit`; Reset Password → `users::reset_password`; Groups
      → `groups::assign_users`; Delete → `users::delete`; active
      Switch → `users::toggle_status`. Self-row + root-admin lockout
      guards on Switch and Delete.
- [x] `CreateUserDrawer`, `EditUserDrawer`, `ResetPasswordDrawer`,
      `AssignGroupDrawer`, `UserGroupsDrawer`: form `disabled`
      derived from corresponding `users::*` / `groups::*` perm; submit
      hidden when missing (Cancel relabeled "Close").
- [x] `UserGroupsSettings`: Create group → `groups::create`; drawer
      same pattern.
- [x] `GroupListItem`: Members → `groups::read`; Edit →
      `groups::edit`; Delete → `groups::delete` (also hidden on
      system groups).
- [x] `EditUserGroupDrawer`: `groups::edit` on form + submit.

### llm-provider — Admin providers + assignments

Backend perms: `llm_providers::*`, `user_llm_providers::read`.

- [ ] `module.tsx`: `settingsAdminPages[llm-providers]` →
      `llm_providers::read`.
- [ ] List + detail: Create → `llm_providers::create`; Edit →
      `llm_providers::edit`; Delete → `llm_providers::delete`;
      Assign to groups → `llm_providers::assign_groups`.

### llm-repository

Backend perms: `llm_repositories::*`.

- [ ] `module.tsx`: `settingsAdminPages[llm-repositories]` →
      `llm_repositories::read`.
- [ ] List: Create / Edit / Delete buttons gated correspondingly.
- [ ] Drawer: form + submit.

### llm-model — Models, downloads

Backend perms: `llm_models::*`,
`llm_models::downloads_{read,cancel,delete}`.

- [ ] Downloads list: Cancel → `llm_models::downloads_cancel`;
      Delete → `llm_models::downloads_delete`. Read-only access via
      `llm_models::downloads_read`.
- [ ] Local model add drawer / upload → `llm_models::create` (or
      `download`).

### mcp — User MCP + System MCP

Backend perms: `mcp_servers::*` (user), `mcp_servers_admin::*`
(system).

- [ ] `module.tsx`: `settingsAdminPages[mcp-admin]` →
      `mcp_servers_admin::read`. (User-mcp page is per-user, no gate.)
- [ ] `SystemMcpServersPage` + system drawer: Create / Edit / Delete /
      Assign-groups gated on `mcp_servers_admin::*`.
- [ ] `McpServersSettings` (user): Create → `mcp_servers::create`;
      Edit → `mcp_servers::edit`; Delete → `mcp_servers::delete`.

### code-sandbox — DONE (migrated, but rollout-row recorded)

- [x] Helpers migrated to `core/permissions`.
- [x] `SandboxEnvironmentsSection`: `code_sandbox::environments::read`
      / `…::manage` via `usePermission`.
- [x] `SandboxResourceLimitsSection`:
      `code_sandbox::resource_limits::manage` form gate.
- [ ] `module.tsx`: `settingsAdminPages[code-sandbox]` →
      `{ anyOf: ['code_sandbox::environments::read',
      'code_sandbox::resource_limits::read'] }` (gate sidebar +
      deep-link, in addition to the in-page gates already present).
- [ ] `SandboxResourceLimitsSection` is also missing the `…::read`
      gate (per audit 06 F-2) — surround the visible card with
      `<Can permission="code_sandbox::resource_limits::read">`.

### assistants — User assistants + admin templates

Backend perms: `assistants::*`, `assistant_templates::*`.

- [ ] `module.tsx`: `settingsAdminPages[assistants]` →
      `assistant_templates::read`. (`/assistants` user route is
      per-user, no admin gate.)
- [ ] `AssistantsSettings` (templates): Create/Edit/Delete →
      `assistant_templates::*`.
- [ ] `UserAssistantsPage`: Create → `assistants::create`; Edit →
      `assistants::edit`; Delete → `assistants::delete`.
- [ ] `AssistantFormDrawer`: form + submit gated.

### hardware

Backend perms: `hardware::read`, `hardware::monitor`.

- [ ] `module.tsx`: `settingsAdminPages[hardware]` →
      `hardware::read`.
- [ ] `HardwareSettings`: monitor toggle / SSE subscribe gated on
      `hardware::monitor`.

### llm-local-runtime

Backend perms: `llm_local_runtime::{read,manage,logs,create,update,delete}`.

- [ ] `module.tsx`: `settingsAdminPages[llm-runtime]` →
      `llm_local_runtime::read`.
- [ ] List + version management: Create/Update/Delete/Logs gated.

### file

Backend perms: `files::{read,upload,download,delete,preview,generate_token}`.

Files are surfaced inline within chat / drawers, not as a settings
page. Audit row primarily covers buttons inside file widgets.

- [ ] Inline file action buttons (download/delete/preview) where
      they appear in chat / drawer surfaces.

### chat

Backend perms: `conversations::*`, `messages::*`, `branches::*`.

Every authenticated user typically has these in the default group.
Gates are LOW priority — most users have them by default.

- [ ] Conversation actions (delete/rename) gated on
      `conversations::delete` / `…::edit`. If always granted to
      default group, mark `[-]`.
- [ ] Message actions (delete/branch) gated on `messages::delete` /
      `branches::create`.

---

## Cross-cutting

- [ ] Test user fixtures (root, admin, member, readonly, partial-hub).
- [ ] Playwright `no-403` detector fixture.
- [ ] `tests/e2e/permissions/<module>.spec.ts` per module.
- [ ] `.claude/PERMISSION_GATING.md` + CLAUDE.md doc-index entry +
      pointer from `REACT_COMPONENT_PATTERNS.md`.

---

## Backend-side cleanup (out-of-scope tickets to file)

- `user/service.rs::has_permission` (legacy single-colon split) →
  align with `permissions/checker.rs` (`::`) or delete.
- `Auth.store` `auth.*` event family (audit 03 B-3) — emit on
  login/register/logout for cache invalidation.
- `EditUserDrawer.tsx` dropped `display_name` field (audit 03 B-1) —
  unrelated bug, fix before gating disable.
- `UserRegistrationSettings` stub (audit 03 B-4) — hide or wire to
  real backend before gating.
- Self / root-admin lockout guards (audit 03 B-6) — beyond
  permission checks, need explicit guards (`user.id === self`,
  `user.is_admin`).
