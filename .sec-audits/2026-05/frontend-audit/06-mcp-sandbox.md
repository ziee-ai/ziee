# MCP module + Code Sandbox audit

## Summary
- 6 HIGH, 9 MED, 8 LOW findings across `src/modules/mcp/` and `src/modules/code-sandbox/`.
- **Permission plan NOT yet applied**: there is no `src/core/permissions/` module in the tree. The two sandbox sections still carry their duplicated local `hasPermission` helper. The helper *does* honor the global `*` wildcard, so root admins (`Administrators` seeded with `['*']`) work, but no `user.is_admin` short-circuit exists — if a future deployment removes the wildcard from the Administrators group or seeds an admin-by-flag account that *doesn't* live in the Administrators group, every sandbox button silently goes read-only (latent regression risk; see F-1).
- **Single biggest correctness bug**: `SandboxResourceLimitsSection` checks `MANAGE` perm only — there is no `READ` gate on the section, so a user with neither read nor manage will still call `GET /resource-limits` and see the form (it just disables Save). Backend rejects with 403, but the section never renders the permission-denied alert that `SandboxEnvironmentsSection` shows. Inconsistent with the sibling card and with the read/manage split documented in `CLAUDE.md`.
- **Largest perf issue**: the MCP "system servers assigned to a group" lookup is N+1 in three places (loops `Group.getServerGroups` once per server). A bulk endpoint `Group.getSystemServers` already exists in the generated client but the UI never calls it. With dozens of system servers this is dozens of round-trips per drawer-open / per group-widget mount.
- **Dual-namespace check passes**: I did not find a system component reaching for `mcp_servers::*` (user namespace) or vice versa. The MCP UI today does not gate any button on permissions strings at all — all gating is done at the route/menu level by Auth, plus per-mode logic (`is_system` driving which store is called). That's a different shape from the sandbox section, but it does mean no namespace-mismatch bugs exist here.

---

## Bugs

### F-1 — `core/permissions` plan NOT applied; sandbox sections still carry the duplicated local `hasPermission` helper, no `is_admin` short-circuit  [HIGH]
**File:** `src/modules/code-sandbox/components/SandboxEnvironmentsSection.tsx:17-23`, `src/modules/code-sandbox/components/SandboxResourceLimitsSection.tsx:30-36`
**What:** The audit task assumed the permission plan was implemented and that both files would now `import { hasPermission } from '../../../core/permissions'`. `find src/core -name '*permission*'` returns nothing — the new module does not exist. Both files still ship the local copy. The local copy *does* honor the global `*` wildcard (line `:18`/`:31`: `if (perms.includes('*')) return true`), which is why root admins work today — `Administrators` is seeded with `['*']` in their permissions array. What it does NOT honor is `user.is_admin === true` as a standalone flag (no access to the `user` object — only `permissions: string[]` is passed). If an admin is provisioned with `is_admin: true` but **not** in the Administrators group (e.g. a future SSO sync that maps OIDC `admin` to `is_admin` but doesn't add group membership, or a root-bootstrap account where the migration sets `is_admin` directly), every sandbox button in this view silently disables and the section shows "you don't have permission" — exactly the latent bug the plan called out.
**Fix sketch:** Either ship the `core/permissions/hasPermission` module (move the helper, accept `user: User | null` and short-circuit on `user?.is_admin`, then import it in both sections), or — if no `is_admin` short-circuit is desired — strip the comment in `SandboxResourceLimitsSection.tsx:24-29` claiming the helper aligns with the backend. The backend `auth/backend.rs::has_permission` also short-circuits on `is_admin` (per the audit prompt); the FE does not.

### F-2 — `SandboxResourceLimitsSection` has no `code_sandbox::resource_limits::read` gate; renders form for users who can't read it  [HIGH]
**File:** `src/modules/code-sandbox/components/SandboxResourceLimitsSection.tsx:22-23`, `:104-108`
**What:** Only `MANAGE_PERM = 'code_sandbox::resource_limits::manage'` is defined and checked. There is no `READ_PERM` constant, no early-return on missing read perm, no `<Alert message="You don't have permission to view resource limits"/>` like the sibling `SandboxEnvironmentsSection` does (`SandboxEnvironmentsSection.tsx:61-71`). Effect:
1. The user opens `/settings/sandbox`.
2. The store's `__init__.limits` fires (`SandboxResourceLimits.store.ts:42-62`) and hits `GET /api/code-sandbox/resource-limits`.
3. Backend returns 403 (per the audit prompt, the BE permission gate is `code_sandbox::resource_limits::read`).
4. `error` is set to "Failed to load resource limits", and the form skeleton briefly renders with `disabled={!canManage}`, then the section renders an `<Alert type="error">` from `:139-147` — but the manage section is still present in the DOM.
5. The user sees a confusing mix of "Failed to load resource limits" alert + a disabled empty form, instead of the clean "You don't have permission" stub that the sibling section ships.
Inconsistent UX between two sections of the same page. Confirmed by grep: the BE permission `code_sandbox::resource_limits::read` exists at `src-app/server/src/modules/code_sandbox/permissions.rs:62` and the 403 wiring at `handlers.rs:1461`; the FE has zero references to it.
**Fix sketch:** Mirror the `EnvironmentsSection` shape exactly: define `const READ_PERM = 'code_sandbox::resource_limits::read'`, compute `canRead = hasPermission(perms, READ_PERM) || canManage`, early-return the permission-denied Alert before rendering the Form.

### F-3 — `SystemMcpServer.store.loadSystemServers` dedup guard skips first-mount concurrent fetches  [LOW]  *(revised 2026-05-23: downgraded from HIGH; framing was wrong)*
**File:** `src/modules/mcp/stores/SystemMcpServer.store.ts:160-166`
**What:**
```ts
if (
  state.systemServersInitialized &&
  state.systemServersLoading &&
  !page
) {
  return
}
```
Reads as: "if already initialized AND currently loading AND no page param → skip." **Re-review:** the guard is actually correct for the post-init concurrent-dedup case (initialized=true && loading=true → skip), which is the primary case it needs to handle. On first mount, `initialized=false` so the guard correctly falls through and proceeds to fetch. The only gap is the rare "concurrent first-mount calls before initialization completes" scenario — but the proxy's `propInitCheck` mechanism in `core/stores.ts:203-209` already prevents this for the auto-init path (init runs once per prop). The race would only manifest from explicit imperative calls to `loadSystemServers()` from multiple call sites in the same tick before the first completes — a theoretical case with no clear reproducer.

**Net:** the guard does its job. The audit's claim "logically impossible to enter on a first mount" is technically true but mischaracterized — the guard is intentionally not active on first mount (you need to fetch). The audit's claim "useless after init" is wrong — that's exactly when the guard fires.

The minor improvement would be `if (state.systemServersLoading && !page) return` which would additionally dedup the theoretical concurrent-first-mount case. Worth doing for defensive coding but not a correctness bug today. Cross-ref: `09-cross-cutting-correctness.md` B-3 made the same overstatement and is being downgraded in parallel.

**Fix sketch (optional defensive):** `if (state.systemServersLoading && !page) { return }`.

### F-4 — N+1 API calls in `getServersForGroup`, `loadServersForGroup`, `loadGroupsForServer` — bulk endpoint `Group.getSystemServers` already exists  [HIGH]
**File:** `src/modules/mcp/stores/SystemMcpServer.store.ts:391-410`, `src/modules/mcp/widgets/GroupSystemMcpServersWidget.store.ts:243-260`, `src/modules/mcp/components/system/McpServerGroupsAssignmentCard.store.ts:194-217`
**What:** Three different code paths walk the entire `systemServers` array calling `McpServerSystem.getServerGroups({ id: server.id })` for each, then filter by `groupIds.includes(groupId)`. With N system servers, that's N HTTP round-trips per drawer-open. The generated API client already exposes `Group.getSystemServers` (`api-client/types.ts:1931`) which returns `{ servers: McpServer[] }` directly. Two of the three call sites are the per-group widgets (mounted in every row of the groups list), so opening the admin Groups page with M groups × N servers = M*N requests on first load.
The `loadGroupsForServer` path (`McpServerGroupsAssignmentCard.store.ts:199`) is slightly different: it asks "what groups own this server" — a single call per server, no loop. That one is fine.
But `loadServersForGroup` (`GroupSystemMcpServersWidget.store.ts:252-260`) and `getServersForGroup` (`SystemMcpServer.store.ts:396-403`) are the offenders. **Both already get the answer in one request via `Group.getSystemServers`.**
**Fix sketch:** Replace the for-loop with `const { servers } = await ApiClient.Group.getSystemServers({ group_id: groupId })`.

### F-5 — `GroupSystemMcpServersAssignmentDrawer` doesn't reset `assignedIds`/`loading` when drawer closes; stale state flashes on reopen  [MED]
**File:** `src/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx:18-44`, `src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:20-46`
**What:** `assignedIds` and `loading` are component-local `useState`. The cleanup-on-close path doesn't reset them — `handleClose` only closes the drawer via the store. The `useEffect` at `:23-27` (and `:25-29` in the sibling) reloads on `[isOpen, selectedGroup]`, but for the brief window between reopen and the new fetch returning, the user sees the previous group's assigned servers pre-toggled. Worst case: admin clicks "Save" instantly on reopen and writes the *previous* group's selection to the *current* group. Race window is short (one round-trip), but real.
**Fix sketch:** Either reset state in `handleClose` (`setAssignedIds([]); setLoading(false)`), or `setAssignedIds([])` at the top of `loadAssignedServers` before the await. The latter shows a clean empty state during the brief reload.

### F-6 — `McpServerDrawer.tsx` mixes "save server + save OAuth config" non-transactionally; OAuth write failure leaves server saved with inconsistent UI  [MED]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:194-267`
**What:** `handleSubmit` first calls `createMcpServer` / `updateMcpServer` (lines 195-244), then *if* it's a user-mode HTTP server, separately calls `ApiClient.McpServer.setOAuthConfig` / `deleteOAuthConfig` (`:247-265`). If the server save succeeds but the OAuth call fails, the `catch (error)` at `:271-274` shows generic `"Failed to save MCP server"` — the user sees a generic failure, doesn't know the server actually was created, doesn't know OAuth wasn't saved. The next attempt re-runs `createMcpServer` and hits a name-uniqueness error. No retry-friendly state model.
**Fix sketch:** Wrap the OAuth call in its own try/catch with a distinct error toast ("Server saved, but OAuth config failed: ..."), so the user knows to retry only OAuth. Also: if `createMcpServer` succeeded and OAuth failed, do NOT `closeMcpServerDrawer()` — keep the drawer open so the user can re-enter the secret and click Save again.

### F-7 — McpServerDrawer footer buttons are NOT in `Drawer.footer` prop; long forms hide the Save button below the fold  [MED]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:525-530`, vs `GroupSystemMcpServersAssignmentDrawer.tsx:81-95` and `McpServerGroupsAssignmentDrawer.tsx:93-107`
**What:** The two assignment drawers correctly use `<Drawer footer={<...>}/>` so Save/Cancel float at the bottom regardless of body scroll. `McpServerDrawer` puts the buttons inline at the end of the body (`<div className="flex gap-2 justify-end">` inside the form region). On a short viewport (e.g. laptop with devtools open) the create-mode form is 600+ px tall — Sampling/Usage Mode/Max Concurrent Sessions all push the buttons off-screen. The user has to scroll past every field to find Save, and there's no way to confirm validation errors without scrolling back up. Inconsistent with the rest of the codebase.
**Fix sketch:** Move the `<Button>Cancel</Button><Button type="primary">Save</Button>` block into `<Drawer footer={...}>` like the sibling drawers.

### F-8 — `useEffect` dep-array misses `loadAssignedServers`/`loadAssignedGroups` (referentially-fresh inner function); React lint warning  [LOW]
**File:** `src/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx:23-27`, `src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:25-29`
**What:** Both effects depend only on `[isOpen, selectedGroup]` / `[isOpen, selectedServerId]` but call a non-memoised closure `loadAssignedServers` / `loadAssignedGroups` defined inline. ESLint `react-hooks/exhaustive-deps` would warn. Not a runtime bug today because the inline function is read on every render and the closure captures the latest `setLoading`/`message`, but the warning gets disabled or muted, and a future refactor that wraps these in `useCallback` would break the effect. Tangentially, `selectedGroup` is a reference-unstable Group object — every re-render of the store (e.g. from a tag flip elsewhere) re-runs the effect even though the *id* didn't change.
**Fix sketch:** Wrap the inner async function in `useCallback`, or — preferred — pull it out into a function that takes `selectedGroup.id` and depend on the id only.

### F-9 — `SystemMcpServersPage` doesn't surface `systemServersError`; failures silently leave the page in a half-loaded state  [MED]
**File:** `src/modules/mcp/components/system/SystemMcpServersPage.tsx:17`, vs `src/modules/mcp/components/user/McpServersSettings.tsx:18-23`
**What:** `McpServersSettings` displays the error via `App.message.error` and clears it (lines 18-23). `SystemMcpServersPage` only reads `{systemServers, systemServersLoading}` — no error pickup, no Alert. If `loadSystemServers` rejects (network, 401, 403), the page shows a perpetual `"Loading system servers..."` text (line 64) and an empty server list, with no signal to the user. The error is also never cleared. Inconsistent with the user-facing page.
**Fix sketch:** Mirror `McpServersSettings.tsx:18-23`: destructure `systemServersError`, show `message.error(systemServersError)` on change, call `Stores.SystemMcpServer.clearSystemMcpErrors()`.

### F-10 — McpServerCard hides enable/disable Switch for system servers in user view → users can't see if a system server is on  [MED]
**File:** `src/modules/mcp/components/common/McpServerCard.tsx:127-138`
**What:** In `McpServersSettings`, system servers are rendered with `isEditable={!server.is_system}` → `isEditable=false`. The `{isEditable && (...)}` block wraps both the Switch (the enabled indicator) AND the Edit button. So a system server in user view shows only the colored transport tag and the `System` tag — no green/grey enabled state. A user looking at "is this server actually on for me?" has no answer. They'd have to chat-test it.
**Fix sketch:** Either always show the Switch (read-only with `disabled` when `!isEditable`) or add a small `Tag` showing "Enabled"/"Disabled" status outside the editable region.

### F-11 — `McpServerCard.handleToggleEnable` doesn't wait for server-side health; toggling can leave UI showing "enabled" while server is unreachable  [LOW]
**File:** `src/modules/mcp/components/common/McpServerCard.tsx:58-77`
**What:** The toggle calls `updateMcpServer({ enabled: true })` and on success says `"Server enabled successfully"`. But "enabled" only means "the row's enabled flag is true" — the actual MCP daemon connection might still fail on first tool call (stdio process won't start, HTTP URL is 404, etc.). The drawer has no "test connection" button either (the audit checklist asks: "Server connection test (does it test? what UI feedback?)" — answer: **no test button anywhere**). Users only learn the server is broken when they try to use it in chat.
**Fix sketch:** Add a `<Button>Test connection</Button>` in the McpServerDrawer that calls a (new) backend health-probe endpoint and surfaces the result. At minimum, after a successful enable, poll the daemon status once and update the toggle on failure.

### F-12 — `GroupSystemMcpServersWidget.store` mutates `state.serversInitialized = false` then awaits `loadAllServers` inside the event handler — race with concurrent reads  [LOW]
**File:** `src/modules/mcp/widgets/GroupSystemMcpServersWidget.store.ts:91-102`
**What:** When `mcp_server.created` fires for a system server, the handler does:
```ts
set(state => { state.serversInitialized = false })
await get().loadAllServers()
```
The `set()` makes `serversInitialized=false` synchronously. Any component re-render that happens between the `set` and the `await` resolution will see `serversInitialized=false` and may itself call `__init__.allServers` → `loadAllServers()` again. The guard in `loadAllServers` at `:165-167` (`if (state.serversLoading) return`) helps but only if the second call lands *after* the first call has started its own `set({ serversLoading: true })`. Microtask ordering makes this brittle. In practice 99% of the time it works, but a duplicate fetch is possible.
**Fix sketch:** Don't toggle `serversInitialized` to false — just call `await get().loadAllServers()` with a `force` flag, or do `set({ serversInitialized: false, serversLoading: true })` in the same set.

### F-13 — `mcp_server.deleted` handler in `GroupSystemMcpServersWidget.store` rebuilds the entire `groupServers` Map even for unaffected groups  [LOW]
**File:** `src/modules/mcp/widgets/GroupSystemMcpServersWidget.store.ts:127-148`
**What:** On any system server delete, the handler iterates every cached group and rebuilds its entry, even if that group never had the deleted server. Cheap (O(groups × servers)) but unnecessary, and it sets every entry to a new object so any selector subscribed via Zustand's `shallow` will re-render every group widget. With dozens of groups in the admin Users page, that's a noticeable hiccup.
**Fix sketch:** Only update entries where `groupData.servers.some(s => s.id === serverId)`.

### F-14 — `McpServerDrawer` doesn't disable the form during in-flight submit beyond the primary button  [LOW]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:316-318`, `:525-530`
**What:** Only the primary "Save" button has `loading={loading}`. The Cancel button, the form fields, the Switch toggles are all live during submission. Clicking "Cancel" mid-save closes the drawer but the API call keeps running; if it fails, the error toast appears with no context for the user. Worse: the form fields can be edited and re-submitted while the previous request is in flight (the loading-set in the store guards `creating` but not the handler).
**Fix sketch:** Add `<Form disabled={loading}>` and `<Button disabled={loading}>Cancel</Button>`; or set the entire drawer's `closable={!loading}`.

### F-15 — OAuth secret field's `placeholder` leaks "stored" state to anyone who can read the form, even if `hasExistingOAuth` is wrong  [LOW]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:458-461`
**What:** `placeholder={hasExistingOAuth ? '•••••••• (unchanged)' : 'client secret'}` — the bullet placeholder visually implies a secret is stored. If `getOAuthConfig` returns falsy due to a network blip, `setHasExistingOAuth(false)` (line 67) and the placeholder becomes `'client secret'`. The user thinks no OAuth is configured, retypes the secret to a blank string, hits Save. Code path at `:262-265` reads "clientId set, secret blank, config exists → keep" — but since `hasExistingOAuth` is *false* due to the load error, line 258 logic instead fires the error "Enter the OAuth client secret to enable OAuth". OK, not a data-loss bug, but a confusing UX.
**Fix sketch:** When `getOAuthConfig` fails, show a `<Spin/>` or a "couldn't load OAuth status, please retry" state instead of silently defaulting to "no OAuth".

### F-16 — Two `McpServerDrawer` `useEffect`s both depend on `[editingServer, open, mode, form]` and run in undefined order on open  [LOW]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:48-75`, `:78-115`
**What:** The first effect loads OAuth config and sets form fields. The second effect populates the rest of the form fields. They both run on the same deps. React doesn't guarantee execution order between separate effects in the same render, but in practice it's "source code order". If a future contributor reorders them or the codebase is upgraded to React Compiler / RSC, the OAuth fields may be set *before* the main `setFieldsValue` clobbers them. The two effects should be merged into one, or the OAuth load should happen as part of the main form-population pass.
**Fix sketch:** Merge into one effect, with the OAuth fetch awaited before `setFieldsValue`.

### F-17 — `McpServer.store.loadMcpServers` lacks an `initialized` check; every event handler re-fetches the full server list  [MED]
**File:** `src/modules/mcp/stores/McpServer.store.ts:131-169`, `:175-208`
**What:** Six event handlers (`mcp_server.groups_changed`, `mcp_server.group_servers_changed`, `group.member_added`, `group.member_removed`) all call `await get().loadMcpServers()`. The store does check `state.loading` to dedupe concurrent calls (`:179`), but does NOT short-circuit on "already initialized recently and no semantic change". A single admin action that fires multiple events (e.g. updating a group's server list → emits `mcp_server.group_servers_changed`; then the backend also broadcasts `mcp_server.groups_changed` per-server) can cascade into multiple full server-list reloads. Compare with `LLMProviderStore` (per the patterns docs) which typically de-dupes by `lastFetched < 30s`.
**Fix sketch:** Add a `lastFetched` timestamp + 30-second skip, like `GroupSystemMcpServersWidget.store.ts:170-181` does. Or coalesce the four event subscribers into one debounced call.

### F-18 — `SystemMcpServer.store.__destroy__` resets `systemServers` to `[]` after 10 s, but in-flight drawers reference `systemServers.find(...)` for `selectedServer`  [MED]
**File:** `src/modules/mcp/stores/SystemMcpServer.store.ts:458-489`, `src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:83-85`
**What:** `__destroy__` resets the store to empty after 10 s with no listeners. The McpServerGroupsAssignmentDrawer in the `components` registry mounts only when `Stores.McpServerGroupsAssignment.isOpen` is true (via `useDelayedFalse` in `mcp/module.tsx:113-114`). If the admin opens the drawer, then navigates away from the System MCP Servers page (which is the page that holds a ref to `Stores.SystemMcpServer.systemServers`), and the drawer is still mid-render, the store's ref count drops to 0 (drawer reads `selectedServerId`, not `systemServers`, so it doesn't keep that proxy property alive). After 10 s of being open, `__destroy__` clears `systemServers` and the drawer's `selectedServer` lookup returns `undefined` → title becomes "Assign User Groups - " (empty server name). Probably never hit in practice but the dependency on cross-store mounted lifetime is fragile.
**Fix sketch:** Move `selectedServer` resolution into the drawer's open-time fetch (load the server details by ID on open), or have `McpServerGroupsAssignment` carry a snapshot of `display_name` along with `selectedServerId`.

---

## Inconsistencies

### F-19 — Drawer titles and labels don't agree on "User Groups" vs "system server" capitalization  [LOW]
**File:** `src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:89`, `src/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx:77`
**What:** First says `"Assign User Groups - {server.display_name}"`. Second says `"Assign System MCP Servers - {group.name}"`. The sentences are not parallel (one uses "User Groups", the other "System MCP Servers" — both correct but visually mismatched in the drawer header). Same module, same admin UI, two different titling conventions.
**Fix sketch:** Pick one — e.g. `"User groups for {server}"` and `"System MCP servers for {group}"` (consistent lowercasing).

### F-20 — `SettingsPage` does not filter `settingsAdminPages` by Auth permissions; non-admin users see the menu item if it's rendered  [HIGH]
**File:** `src/modules/settings/SettingsPage.tsx:22-50`
**What:** The Settings sidebar shows every `settingsAdminPages` slot unconditionally. The route itself is guarded with `requiresAuth: true` (`mcp/module.tsx:67`, `code-sandbox/module.tsx:27`) but `requiresAuth` only checks `isAuthenticated`, not permissions. A logged-in non-admin user clicking "System MCP Servers" / "Code Sandbox" in the menu lands on the page, which then either shows an empty state or a 403-toast. The UX implies access where there is none; worse, it surfaces the existence of admin-only features to all users. Cross-cuts more than just MCP/sandbox, but the symptom is most acute for these two pages because they're the newest and least permission-aware.
**Fix sketch:** Extend `SettingsPageSlot` with an optional `requiresPermission: string | string[]` and have `SettingsPage.tsx` filter using the same `hasPermission` helper (once F-1 ships a shared one).

### F-21 — User-side `McpServersSettings` shows a "User"/"System" filter but the user has no way to manage system servers from there  [LOW]
**File:** `src/modules/mcp/components/user/McpServersSettings.tsx:132-138`
**What:** The status filter offers `{ label: 'System', value: 'system' }`. Selecting it shows only system servers, all of which render with `isEditable=false` (no Edit/Delete/Switch). So the filter is informational only. Fine, but other modules typically hide filters that produce non-interactive results — or label them differently ("Available system servers").
**Fix sketch:** Either remove the system-only filter or rename it to "System (read-only)".

### F-22 — `SystemMcpServersPage` doesn't accept a `clone` mode the drawer supports  [LOW]
**File:** `src/modules/mcp/components/system/SystemMcpServersPage.tsx:24-26`, `src/modules/mcp/stores/McpServerDrawer.store.ts:12-17`
**What:** The drawer store's `mode` type includes `'clone'`, but neither user nor system page exposes a Clone button. Dead code in the type union. McpServerDrawer also has no UI branch for `mode === 'clone'`. Either implement it (clone-to-create with a "Clone of X" pre-filled name) or remove from the type.
**Fix sketch:** Remove `'clone'` from the type if no plans to use it.

### F-23 — `SandboxEnvironmentsSection` SSE columns use inline 180-px width, others use `flex` — looks misaligned on narrow viewports  [LOW]
**File:** `src/modules/code-sandbox/components/SandboxEnvironmentsSection.tsx:124-132`
**What:** The "Status" cell has `style={{ minWidth: 180 }}` when a fetch is in progress, but other states (Cached tag, Failed tag, Not fetched tag) have no minWidth. Result: the Status column's width snaps when a row transitions to/from `running`. Visually janky during prefetch.
**Fix sketch:** Set a column-level `width: 200` so the cell width is stable regardless of phase.

### F-24 — `SandboxResourceLimitsSection` only treats `error` from initial load; a save error sets `error` but isn't visually distinct from a load error  [LOW]
**File:** `src/modules/code-sandbox/components/SandboxResourceLimitsSection.tsx:139-147`, `src/modules/code-sandbox/stores/SandboxResourceLimits.store.ts:95-101`
**What:** Both the load path (`:55-60`) and the save path (`:96-100`) set `s.error` to a generic string. The UI shows `<Alert message="Failed to load resource limits" description={error}/>` even if the error was actually a save failure. The store also has `loading: true` set on save (`:85-87`), so the form briefly switches to the Spin tip. Confusing during error recovery.
**Fix sketch:** Split the store into `loadError` / `saveError`; render distinct alerts.

---

## Responsive

### F-25 — McpServerCard headerBg `flex-wrap` collapses transport tag rows but doesn't reserve space for the action buttons → buttons overflow under tags on narrow viewports  [MED]
**File:** `src/modules/mcp/components/common/McpServerCard.tsx:97-126`
**What:** The header `<div className="-mx-3 -mt-3 mb-3 px-3 py-2 flex items-center gap-2 flex-wrap ...">` puts the title block + transport tags + sampling/always tags on one row, and the Switch/Edit/Delete buttons on the same row. With many tags (Sampling + Always + System + transport), the buttons wrap to a second row but lose right-alignment because the inner `<div className="flex gap-2 items-center justify-end">` has no `flex-shrink-0`. Result: at 640 px width, buttons stack vertically below the title with `justify-end` still trying to push them right — looks unintentional.
**Fix sketch:** Wrap the title in one `<div className="flex-1 min-w-0">` and the actions in `<div className="flex-shrink-0">`. Or break to a column layout below `md`.

### F-26 — McpServerDrawer transport-specific fields have no flow/wrap considerations; HTTP Headers TextArea overflows on mobile  [LOW]
**File:** `src/modules/mcp/components/common/McpServerDrawer.tsx:425-435`
**What:** `<TextArea rows={4} className="font-mono text-xs">` with the JSON placeholder is fine on desktop, but on a 375-px viewport the placeholder text wraps awkwardly inside the textarea, and the Drawer `size={600}` is wider than the viewport — Ant Design's drawer caps at viewport width, so the form fields squeeze. Acceptable for an admin-only page, but worth noting.
**Fix sketch:** Use `Drawer size={'min(600px, 90vw)'}` or rely on the parent layout's mobile-aware Drawer wrapper.

### F-27 — Filter row in `McpServersSettings` and `SystemMcpServersPage` uses `flex-wrap` without ordering hints; on narrow screens the "Add Server" primary action ends up last visually, hard to find  [LOW]
**File:** `src/modules/mcp/components/user/McpServersSettings.tsx:115-160`, `src/modules/mcp/components/system/SystemMcpServersPage.tsx:67-109`
**What:** The Add button is the right-most flex child. On flex-wrap, secondary controls (search input, filters) wrap first and the primary CTA jumps onto a new row at the bottom. Typical UX expectation is the primary CTA stays prominent.
**Fix sketch:** Make Add button `order: -1` on small screens, or place it in a sticky header on mobile.

### F-28 — `SandboxResourceLimitsSection`'s Row/Col grid (`Col span={8}` × 3) breaks below 720 px because Ant's `Row` doesn't auto-stack `span={8}` cols  [LOW]
**File:** `src/modules/code-sandbox/components/SandboxResourceLimitsSection.tsx:171-215`, etc.
**What:** Ant's `Col span={8}` is fixed 1/3 width. At 480 px viewport the three fields squish to <160 px each, with labels overflowing. No `xs={24} sm={12} md={8}` responsive breakpoints set anywhere in this file.
**Fix sketch:** `<Col xs={24} sm={12} md={8}>` per field.

---

## Inefficiencies

### F-29 — `SystemMcpServer.store` has both an `immer`-less plain Zustand creator AND another store (`McpServer.store`) calls `useSystemMcpServersStore.setState(state => { ... })` from inside immer middleware  [MED]
**File:** `src/modules/mcp/stores/SystemMcpServer.store.ts:73-75`, `src/modules/mcp/stores/McpServer.store.ts:281-294`, `:336-341`, `:368-381`
**What:** `SystemMcpServer.store` is plain `create<...>()(subscribeWithSelector((set, get) => ({...})))` — no immer. `McpServer.store` does use immer. When `McpServer.store.updateMcpServer` updates `useSystemMcpServersStore.setState(state => { ... return state})`, it returns the same state object back (with `.map` producing a new array). Fine, but mixed patterns make refactors error-prone, and cross-store mutation from another store's action is the inverse of the event-driven invariant the rest of the codebase tries to maintain. The `mcp_server.updated` event already handles the system case correctly inside the SystemMcpServer subscriber — the explicit cross-store `setState` is redundant and competes with the event handler.
**Fix sketch:** Drop the explicit `useSystemMcpServersStore.setState` calls in `McpServer.store.updateMcpServer`/`deleteMcpServer`/`getMcpServer`. Rely on the event subscriber.

### F-30 — `__init__.__store__` calls in widget stores re-subscribe to events with `removeGroupListeners` cleanup but never validate uniqueness — multiple proxy reads can trigger double init under StrictMode  [MED]
**File:** `src/modules/mcp/widgets/GroupSystemMcpServersWidget.store.ts:55-149`, `src/modules/mcp/components/system/McpServerGroupsAssignmentCard.store.ts:55-100`
**What:** The `__store__` callback registers four event listeners under a `GROUP` name. Under React StrictMode double-mount, the proxy ref-tracker (per Audit-1 finding B-2) re-fires `__init__` after the cleanup. Each re-init reads the same `GROUP` string and `eventBus.on(...)` is presumed to dedupe by group — but a quick scan of `removeGroupListeners` shows it removes-by-group, not register-by-group-unique. The subscriber count grows by N on each re-init cycle. In dev, every save fires the listener N×, causing duplicate fetches.
**Fix sketch:** Have `eventBus.on(eventType, handler, groupName)` check if `groupName` is already registered for that event and short-circuit if so. Or track an initialized flag in the store itself.

### F-31 — `loadEnvironments` is called once in `__init__.environments`, AND `evictEnvironment` calls it again on success even though `evictEnvironment` already returns the refreshed list  [LOW]
**File:** `src/modules/code-sandbox/stores/SandboxEnvironments.store.ts:167-188`
**What:** `evictEnvironment` does `const res = await ApiClient.CodeSandbox.evictEnvironment(...)` (line 175), then immediately `set(s => { s.environments = res.available; ... })` (line 178). That's good. But it then does NOT call `loadEnvironments` again. Compare to `subscribeToEvents.complete` (line 153) which DOES call `void get().loadEnvironments()` even though the SSE `complete` event has the new state in `d.bytes_downloaded`. Asymmetric: evict reuses the response payload, complete re-fetches. The complete path could just call `loadEnvironments` once via debounce; currently if two flavors complete in the same tick, the FE makes two redundant GET /environments calls.
**Fix sketch:** Debounce `loadEnvironments` (lodash.debounce or a simple in-flight guard) and let both `evict` and `complete` use it.

### F-32 — `McpServersSettings` does `.filter(...).sort(...)` over `servers` on every render with no `useMemo`; rebuilds for any unrelated state change  [LOW]
**File:** `src/modules/mcp/components/user/McpServersSettings.tsx:35-65`
**What:** `filteredServers` is computed inline. Every keystroke in `searchTerm`/`statusFilter`/`sortBy` is fine (the state change is the trigger), but every typing in some *other* part of the app that re-renders the parent (e.g. theme toggle, route change) also re-runs the full filter+sort. With dozens of servers it's negligible; with hundreds it adds up.
**Fix sketch:** `const filteredServers = useMemo(() => servers.filter(...).sort(...), [servers, searchTerm, statusFilter, sortBy])`.

### F-33 — `phasePercent` returns hardcoded 10/50/75/85/95 but `default` is 5; doesn't match the actual SSE phases the backend emits  [LOW]
**File:** `src/modules/code-sandbox/components/SandboxEnvironmentsSection.tsx:28-43`
**What:** No semantic issue — the progress bar is "coarse stepped" as the comment notes (line 30-32). But "default: 5" runs whenever phase is missing, which happens for the very first SSE message before any `progress` event (line 138's fallback). User sees the bar at 5% for an unbounded time during the "before resolving" window (could be hundreds of ms). Minor UX nit.
**Fix sketch:** Default to 0; or interpret missing-phase as "connecting" with a Spin instead of a Progress.

---

## What was checked and looks fine

- **Dual-namespace correctness**: The MCP UI currently doesn't gate any specific permission string at the component level — gating is per-page (route) + per-mode (`is_system` drives which store/api is used). Consequently, the audit checklist's namespace-confusion concern doesn't manifest. The `user/` components call user stores (`Stores.McpServer.*`), the `system/` components call system stores (`Stores.SystemMcpServer.*`). No crossed wires found.
- **Event-only widget fix for `McpServerGroupsAssignmentCard.tsx`**: `:27-29` correctly fires `useEffect(() => { Stores.SystemMcpServerGroupCard.loadGroupsForServer(serverId) }, [serverId])` AND the store subscribes to events. After page reload the data does load. Matches the `LLMProviderGroupWidget` fix shape.
- **Event-only widget fix for `GroupSystemMcpServersWidget.tsx`**: `:28-30` correctly fires `useEffect(() => { Stores.GroupSystemMcpServersWidget.loadServersForGroup(group.id) }, [group.id])`. Reload-resilient.
- **Sandbox SSE backend integration**: `SandboxEnvironmentsSection`'s `resumeRunningTasks` + `subscribeToEvents` correctly re-attach on mount, the SSE controller registry prevents double-subscribing the same flavor (`:122`), and the cleanup path on completion/failure is paired with `cleanupSse`.
- **OAuth UX scaffolding** (apart from F-15): the keep/replace/remove decision tree at `McpServerDrawer.tsx:247-267` is sound — distinguishes "new OAuth + secret" / "no secret but config exists" / "cleared client id, remove config" cleanly. The placeholder hint (`'•••••••• (unchanged)'`) is the right shape; just needs the load-error case (F-15).
- **Event emitter coverage**: every mutation in both `McpServer.store` and `SystemMcpServer.store` emits the correct event (`mcp_server.created/updated/deleted/groups_changed/group_servers_changed`). Event types are declaration-merged into `AppEvents` in `events/types.ts:48-56`.
