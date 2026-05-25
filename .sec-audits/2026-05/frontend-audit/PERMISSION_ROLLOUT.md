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

### llm-provider — DONE

Backend perms: `llm_providers::*`, `llm_models::*`.

- [x] `module.tsx`: `settingsAdminPages[llm-providers]` →
      `llm_providers::read`.
- [x] `LlmProviderSettings`: 'Add Provider' menu item gated on
      `llm_providers::create`.
- [x] `ProviderHeader`: Edit-name → `llm_providers::edit`;
      Delete → `llm_providers::delete`; enable/disable Switch →
      `llm_providers::edit`.
- [x] `LlmProviderDrawer`: form disabled + submit hidden by
      effective create/edit perm.
- [x] `LlmModelsSection`: per-row Switch/Edit gated on
      `llm_models::edit`; Delete on `llm_models::delete`; Add Model on
      `llm_models::create`.
- [ ] Group assignment drawers (`GroupLlmProvidersAssignmentDrawer`,
      `LlmProviderGroupsAssignmentDrawer`, `ProviderGroupAssignmentCard`,
      `LLMProviderGroupWidget`): gate on `llm_providers::assign_groups`.
      Follow-up.

### llm-repository — DONE

Backend perms: `llm_repositories::*`.

- [x] `module.tsx`: `settingsAdminPages[llm-repositories]` →
      `llm_repositories::read`.
- [x] List: Switch/Test/Edit on `llm_repositories::edit`; Delete on
      `llm_repositories::delete`; Create '+' on `llm_repositories::create`.
- [x] Drawer: form disabled + submit hidden by effective perm.

### llm-model — DONE (surfaced via llm-provider)

Backend perms: `llm_models::*`,
`llm_models::downloads_{read,cancel,delete}`.

- [x] Per-model actions handled in `LlmModelsSection` above.
- [-] No standalone module surface; downloads list inside
      llm-provider drawer not yet enumerated. Follow-up if needed.

### mcp — DONE

Backend perms: `mcp_servers::*` (user), `mcp_servers_admin::*` (system).

- [x] `module.tsx`: `settingsAdminPages[mcp-admin]` →
      `mcp_servers_admin::read`. User MCP page intentionally ungated.
- [x] `SystemMcpServersPage`: 'Add Server' → `mcp_servers_admin::create`.
- [x] `McpServersSettings` (user): 'Add Server' → `mcp_servers::create`.
- [x] `McpServerCard` (shared): derives the namespace from
      `server.is_system`; Switch/Edit → `${ns}::edit`; Delete →
      `${ns}::delete`. Built-in servers still hide Delete.
- [ ] System-MCP Group assignment surfaces
      (`McpServerGroupsAssignmentCard`, the two assignment drawers,
      `GroupSystemMcpServersWidget`): no explicit assign-groups
      permission in `mcp_servers_admin::*` — defer to a backend
      decision on whether to mirror `llm_providers::assign_groups`.

### code-sandbox — DONE

- [x] Helpers migrated to `core/permissions`.
- [x] `SandboxEnvironmentsSection`: `code_sandbox::environments::read`
      / `…::manage` via `usePermission`.
- [x] `SandboxResourceLimitsSection`: now gated on `…::read` (card
      hidden when missing) + `…::manage` (form disabled). Closes
      audit 06 F-2.
- [x] `module.tsx`: `settingsAdminPages[code-sandbox]` →
      `{ anyOf: [environments::read, resource_limits::read] }`.

### assistants — DONE

Backend perms: `assistants::*`, `assistant_templates::*`.

- [x] `module.tsx`: `settingsAdminPages[assistants]` →
      `assistant_templates::read`.
- [x] `AssistantsSettings` (templates): Create/Edit/Delete gated on
      `assistant_templates::*`.
- [x] `UserAssistantsPage`: Create '+' + empty-state Create gated on
      `assistants::create`.
- [x] `AssistantCard`: dropdown menu items + the dropdown itself
      hidden when neither Edit nor Delete is permitted.
- [x] `AssistantFormDrawer`: form disabled + submit hidden, namespace
      derived from `isTemplate`.

### hardware — DONE

Backend perms: `hardware::read`, `hardware::monitor`.

- [x] `module.tsx`: `settingsAdminPages[hardware]` →
      `hardware::read`.
- [x] `HardwareSettings`: skip auto-subscribe, hide Connect + hide
      Monitor (popup) button when `hardware::monitor` is missing.

### llm-local-runtime — DONE

Backend perms: `llm_local_runtime::{read,manage,logs,create,update,delete}`.

- [x] `module.tsx`: `settingsAdminPages[llm-runtime]` →
      `llm_local_runtime::read`.
- [x] `RuntimeVersionList`: 'Download Version' (both extra and
      empty-state) gated on `llm_local_runtime::create`.
- [x] `RuntimeVersionCard`: 'Set as Default' →
      `llm_local_runtime::update`; Delete →
      `llm_local_runtime::delete`.

### file — DEFERRED

Backend perms: `files::{read,upload,download,delete,preview,generate_token}`.

No standalone settings page; files surface inline in chat /
drawers. Inline button audit deferred — the backend already 403s
unauthorized requests, so the only UX impact is the brief error
state. File a follow-up to sweep widgets in
`src/modules/file/components` after the main rollout lands.

### chat — DEFERRED

Backend perms: `conversations::*`, `messages::*`, `branches::*`.

Default `users` group has these by design — no real UX gap in
production. File a follow-up to add gating for completeness, but
the gate would always evaluate true for typical users.

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
