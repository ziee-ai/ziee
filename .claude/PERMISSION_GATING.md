# Permission Gating

This document is the canonical reference for hiding/disabling UI
based on the current user's permissions. **Read this before adding
admin features, action buttons, or settings pages** — the gating
pattern below is required, not optional.

A missed gate is a UX bug (the user sees a button that 403s on
click), not a security hole — the backend's
`RequirePermissions<…>` extractor is the actual authorization
boundary. This UI layer exists so non-permitted users don't see
controls they can't use.

---

## Mental model

Three layers of gating, in order of preference:

1. **Slot field (declarative).** If the surface is registered through
   the module system (admin settings pages, sidebar entries, hub
   tabs), add a `permission?: PermissionExpr` field to the slot
   entry. The slot consumer handles menu filtering and deep-link 403
   in one place. **Use this whenever possible.**
2. **`<Can>` wrapper (component).** For per-button gates inside a
   page (Create, Edit, Delete, etc.) where the button is a discrete
   element you can wrap. Renders `null` when denied — the button
   simply isn't in the DOM.
3. **`usePermission` hook (boolean).** For conditional logic that
   doesn't fit a wrapper — ternaries with fallback content,
   `disabled` props on form fields, building action arrays from a
   list, conditionally including dropdown menu items.

---

## The `Permission` and `PermissionExpr` types

```ts
import { Permissions } from '@/api-client/types'

export type Permission = Permissions

export type PermissionExpr =
  | Permission
  | { allOf: PermissionExpr[] }
  | { anyOf: PermissionExpr[] }
```

- **`Permissions.*` enum** is generated from the backend OpenAPI spec
  by `openapi/generate-endpoints.ts`, which extracts each endpoint's
  structured `403.required_permissions` example. Strictly typed —
  raw strings are rejected at compile time.
- `allOf: [a, b]` = AND (every child must pass). Empty `allOf` is
  vacuously true.
- `anyOf: [a, b]` = OR (at least one child must pass). Empty `anyOf` is
  false.
- The types nest. Pass them through any of the gating primitives.

Real examples from the codebase:

```ts
// 90% case — single enum member
permission: Permissions.UsersDelete

// AND — page renders content from two backend modules
permission: { allOf: [Permissions.UsersRead, Permissions.GroupsRead] }

// OR — Hub sidebar entry should appear if user can see ANY tab
permission: { anyOf: [
  Permissions.HubModelsRead,
  Permissions.HubAssistantsRead,
  Permissions.HubMCPServersRead,
]}
```

### If a permission isn't in the enum

The generator can only see permissions that appear in the OpenAPI
spec under
`responses.403.content.application/json.example.details.required_permissions`.
That structured example is auto-attached by the backend
`with_permission::<…>()` helper. Two failure modes keep an
endpoint's perm out of the enum:

1. **The handler's `*_docs` function never calls `with_permission`.**
   Fix: call it. `with_permission::<(MyPerm,)>(op)` is the first
   line; everything else is chained on the returned operation.
2. **The `*_docs` function calls `with_permission` AND ALSO
   `.response::<403, ()>()` or `.response_with::<403, (), _>(…)`.**
   Those calls override the structured 403 from `with_permission`
   with an empty body. Fix: delete the redundant 403 override.
   `with_permission` already documents the 403.

After fixing, regenerate:
```bash
cd src-app/server && cargo build && \
  CONFIG_FILE=config/dev.yaml ./target/debug/ziee-chat --generate-openapi
cd ../ui && npm run generate-openapi
```

### Dynamic namespaces

If a component picks the permission namespace at runtime (e.g. a
shared card serving both user and admin modes based on a flag), use
a lookup map on enum members instead of a template-string permission:

```ts
const SYSTEM_PERMS = {
  edit: Permissions.McpServersAdminEdit,
  delete: Permissions.McpServersAdminDelete,
} as const
const USER_PERMS = {
  edit: Permissions.McpServersEdit,
  delete: Permissions.McpServersDelete,
} as const

const perms = server.is_system ? SYSTEM_PERMS : USER_PERMS
const canEdit = usePermission(perms.edit)
```

Template strings like `${ns}::edit` are NOT type-safe (TypeScript
can't narrow string concatenation to enum members) and bypass the
gating guarantee. Always look up enum members through a discriminated
const.

A `not` variant is intentionally omitted — no current surface needs
it. Add it later as `{ not: PermissionExpr }` non-breakingly if a
real use case appears.

---

## Wildcards and `is_admin`

Backend semantics (`src-app/server/src/modules/permissions/checker.rs`,
mirrored exactly in `src/core/permissions/hasPermission.ts`):

- **`*` global wildcard** in a user's permissions grants everything.
  Seeded on the Administrators group; matches anything.
- **`module::resource::*`** hierarchical wildcard grants everything
  under that prefix. For a required permission `a::b::c`, the helper
  checks for `a::*`, then `a::b::*`. **Always `::` (double colon)**,
  not `:`. An earlier sandbox-local helper used `:` and silently
  failed; that's been fixed.
- **`is_admin = true` on the user** short-circuits to allowed. This
  is the **root admin** — a single bootstrap superuser enforced by a
  partial unique index on the users table. See "Root admin vs
  Administrators group" below.

---

## Root admin vs Administrators group

The word "admin" colloquially conflates two distinct concepts:

- **Root admin** (`users.is_admin = true`). A boolean column with a
  partial unique index — at most ONE user system-wide. Set during
  initial setup; not assignable through normal flows. Embedded in
  JWT claims. Bypasses every permission check, including via the
  separate `RequireRootAdmin` extractor for operations even
  Administrators-group members can't perform.
- **Administrators group**. A regular row in the `groups` table
  seeded with `permissions: ['*']`. Multiple members. Revocable
  per-user. This is "make Alice an admin."

`/api/auth/me` returns the **literal** union of explicit user +
group permissions. It does **not** rewrite `permissions[]` to
`['*']` for root admins. The frontend helper short-circuits on
`user.is_admin` **before** evaluating `permissions[]`, so a root
admin with no group membership still works.

Never gate UI on `user.is_admin` directly except for genuinely
root-admin-only surfaces (which today is just the root admin's own
profile row). Use the actual permission strings — that automatically
covers Administrators-group members too.

---

## The primitives

### `core/permissions/types.ts` — `PermissionExpr`

The shared expression type. Already covered above.

### `core/permissions/hasPermission.ts` — leaf check

Pure function. `(user, permissions[], requiredString) → boolean`.
Mirrors `checker.rs::check_permissions_array` plus the `is_admin`
short-circuit. Don't reimplement this elsewhere.

### `core/permissions/evaluatePermission.ts` — tree walker

Pure function. `(user, permissions[], expr) → boolean`. Recurses on
`PermissionExpr`; bare strings delegate to `hasPermission`. Use this
in code paths where you need to evaluate an expression without a
React hook (slot filters, route consumers).

### `core/permissions/usePermission.ts` — React hook

```ts
const canEdit = usePermission(Permissions.UsersEdit)
const canManageSandbox = usePermission({
  allOf: [
    'code_sandbox::environments::manage',
    'code_sandbox::resource_limits::manage',
  ],
})
```

Reads from `Stores.Auth` reactively. Returns boolean. Use for
conditional logic with multiple branches or `disabled` props.

### `core/permissions/Can.tsx` — declarative wrapper

```tsx
<Can permission={Permissions.UsersDelete}>
  <Button danger onClick={handleDelete}>Delete</Button>
</Can>

<Can permission={{
  anyOf: [Permissions.UsersEdit, Permissions.UsersResetPassword]
}}>
  <UserActionsMenu user={user} />
</Can>
```

Renders `null` when denied. Optional `fallback={…}` prop for cases
where a "you don't have access" stub should still be visible
(narrow — most call sites want it gone).

---

## Slot field gating (the declarative path)

Settings pages, hub tabs, and sidebar entries are slot-registered.
The slot types carry an optional `permission` field; consumers
filter on it.

### Settings page

```tsx
// src/modules/users/module.tsx
import { Permissions } from '@/api-client/types'

slots: {
  settingsAdminPages: [{
    id: 'users',
    icon: <UserOutlined />,
    label: 'Users',
    path: 'users',
    order: 10,
    permission: Permissions.UsersRead,  // ← gate here
  }],
}
```

Two things happen automatically:

- `SettingsPage.tsx` filters the menu — non-permitted users don't
  see "Users" in the settings sidebar.
- A deep-link to `/settings/users` from a non-permitted user
  renders an inline `<Result status="403">` in the content area
  (URL preserved).

### Sidebar (navigation + tools)

```tsx
sidebarNavigation: [{
  id: 'hub',
  label: 'Hub',
  path: '/hub',
  icon: <HubIcon />,
  order: 50,
  permission: { anyOf: [
    Permissions.HubModelsRead,
    Permissions.HubAssistantsRead,
    Permissions.HubMCPServersRead,
  ]},
}]
```

`LeftSidebar.tsx` filters both `sidebarNavigation` and
`sidebarTools` slots. Non-permitted entries don't appear.

### Hub tabs (multi-verb)

`HubTabSlot.permissions: { read, refresh? }` — two distinct verbs.
The shell filters tabs by `read` and gates the per-tab Refresh
button on `refresh`. This is the generalized pattern for any slot
whose consumer applies multiple action gates per entry.

---

## Adding a new feature (checklist)

1. **Backend:** Define the permission in
   `src-app/server/src/modules/<m>/permissions.rs`. Gate the endpoint
   with `RequirePermissions<(…)>` in the handler signature.
2. **Frontend, slot-registered surface:** Set `permission: '<m>::…'`
   on the slot entry in `module.tsx`. Done.
3. **Frontend, action button:** Wrap with `<Can permission="…">`.
   For form controls (Edit drawer inputs), derive
   `canEdit = usePermission('<m>::edit')` once at the top and pass
   `disabled={!canEdit}` to the `<Form>`. Hide the submit button
   inside the same `<Can>` block.
4. **Mirror the backend string verbatim.** Open the relevant
   `permissions.rs` and copy the `PERMISSION:` constant exactly. A
   typo silently fails closed (the UI hides the button forever) and
   is hard to catch in review.
5. **Update the audit doc** at
   `.sec-audits/2026-05/frontend-audit/PERMISSION_ROLLOUT.md` —
   add a row for the new surface.
6. **E2E test:** Add an assertion to
   `tests/e2e/permissions/<module>.spec.ts` that a non-permitted
   user doesn't see the new control. Patterns in the
   `tests/e2e/permissions/` README.

---

## Anti-patterns

- **Don't reimplement `hasPermission` locally.** The sandbox section
  had a duplicated helper with a wrong separator (`:` vs `::`) and
  no `is_admin` short-circuit; the bug was silent. Import from
  `core/permissions`.
- **Don't gate on `user.is_admin` for non-root-admin-only features.**
  `is_admin` is the singular root admin. Use the actual permission
  string instead — that covers Administrators-group members too.
- **Don't hard-code `permissions.includes('foo::bar')`.** It misses
  wildcards and the `is_admin` short-circuit. Go through the
  evaluator.
- **Don't show a button and let the backend 403.** The user has no
  way to know they lack permission until they click. Either gate it
  with `<Can>` or hide it for a documented reason.
- **Don't gate routes via a wrapper if they're slot-registered.**
  The slot consumer already handles deep-link 403; double-wrapping
  is duplication.

---

## Worked example: Hub module

The Hub is the reference pattern for complex modules. It has every
shape — slot-driven tabs, a shell that doesn't know its submodules
statically, multi-verb permissions per tab, and per-card action
buttons.

- **Shell sidebar entry** lists the three submodule reads in an
  `anyOf` (`modules/hub/module.tsx`). Users with access to zero
  hub resources don't see "Hub" in the sidebar.
- **`HubTabSlot.permissions`** carries `{ read, refresh? }` per
  tab (`modules/hub/types/HubTabSlot.ts`).
- **Each submodule** (mcp, llm-models, assistants) declares both
  perms when registering its tab.
- **`HubPage.tsx`** filters `visibleTabs` by `permissions.read`,
  renders inline 403 when no tabs are visible OR the URL targets a
  forbidden tab, and conditions the Refresh button on the current
  tab's `permissions.refresh`.
- **Cards** (`McpServerHubCard`, `ModelHubCard`, `AssistantHubCard`)
  use `usePermission` for the Install/Download/Use ternary because
  there's a fallback "View" path for already-installed resources.

The net cost was small: 3 submodule registrations (4-line change
each), one shell page (~10 lines of permission logic), three card
components (1 hook + ternary), one sidebar entry.

---

## Helper unit tests

Not currently set up — the UI project ships only Playwright (E2E)
tests, no unit test runner. The helpers are simple enough that bugs
surface immediately in E2E via the "no-403 in network" detector
fixture. If a unit-test runner is later introduced (vitest is the
natural choice for a Vite project), the helpers are pure and trivial
to cover exhaustively:

- `hasPermission`: `is_admin`, `*`, `module::*`, `module::resource::*`
  (every prefix depth), exact match, missing perms, empty array, null
  user.
- `evaluatePermission`: bare string delegates to leaf; `allOf: []`
  true; `anyOf: []` false; nested mixed trees.

---

## E2E coverage

`tests/e2e/permissions/` contains:

- A `no-403.ts` fixture that fails the test if any `/api/*` response
  returns 403. The highest-leverage regression catcher — converts
  the unbounded "did we miss a gate?" question into a CI signal.
- Per-module specs that log in as a non-admin user fixture and
  assert that specific controls / pages / inputs aren't visible.

See the README in that folder for the test user fixtures (root,
admin, member, readonly, partial) and the assertion patterns.
