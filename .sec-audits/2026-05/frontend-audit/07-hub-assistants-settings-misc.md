# Frontend Audit — Hub, Assistants, Settings, Hardware, Projects, Config-Client

**Agent:** 7 of 9
**Date:** 2026-05-23
**Branch:** `security/remediation-2026-05`
**Scope:** `src-app/ui/src/modules/{hub,assistants,settings,settings-general,hardware,projects,config-client}/`
**Lenses (priority order):** Bugs → Inconsistencies → Responsive → Inefficiencies
**Read-only:** No edits made to `src-app/`. Only file written is this audit.

> **Plan-status note** — The "permission plan" (HubPage filtering, Settings inline-403, etc.) is **NOT yet applied** in this branch. `HubPage.tsx:26` still contains `// TODO: integrate permission check`, `HubTabSlot.ts:9` still uses `permission?: string` (not `permissions: { read; refresh? }`), `SettingsPage.tsx` does not filter by `evaluatePermission`, and `HardwareSettings.tsx:37` has `// TODO: Check hardware::monitor permission`. Findings are written against the **current (pre-plan)** state, with explicit flags where the gap is the plan itself.

---

## Findings

### HIGH-1 — Hub: `visibleTabs` is unfiltered; permission gate never runs
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:26-30`
**Severity:** HIGH (security / inconsistency with `module.tsx` declarations)

`HubPage` declares the intent but bypasses the filter:

```tsx
// Filter by permissions (TODO: integrate permission check)
const visibleTabs = hubTabs
```

Consequence:
1. Every hub tab (`hub::models::read`, `hub::assistants::read`, `hub::mcp_servers::read`) is rendered to every logged-in user regardless of permission. The submodule `module.tsx` files DO carry the per-tab `permission` field (`module.tsx:37`, `:34`, `:39`), but nothing reads it.
2. Deep-links to `/hub/<tab>` for a tab the user lacks reads-permission for render the tab body — there is no 403 fallback or empty-tab guard (`HubPage.tsx:55, 144-156`).
3. The "Refresh" button (`HubPage.tsx:133-140`) is always shown — there is no per-tab `refresh` permission gate.
4. The sidebar entry (`hub/module.tsx:28-37`) has no `anyOf` predicate; a user with zero hub read permissions still sees the Hub link and lands on an empty page that quietly redirects to the first tab.

This is the gap the plan was specifically written for; it is unimplemented in this branch. Server-side route gates are the last defense, but a user landing on a permission-denied tab will see whatever error the API surfaces — typically a swallowed 403 in `loadModels()`/`loadAssistants()`/`loadServers()` (`hub-models-store.ts:63`, `hub-assistants-store.ts:60`, `hub-mcp-servers-store.ts:57`), where the error path just stores the message and the Spin keeps churning until the user sees a "Failed to load X" toast — a poor UX vs the intended inline 403 panel.

**Recommendation:** Apply the plan. Specifically:
- Widen `HubTabSlot.permission?: string` → `permissions: { read: string; refresh?: string }` (`types/HubTabSlot.ts:9-10`).
- Have submodule modules register both keys.
- Filter `visibleTabs` via `evaluatePermission(user, perms, t.permissions.read)` in HubPage.
- Render an inline `<Result status="403">` when `urlActiveTab` resolves to a tab not in `visibleTabs`.
- Conditionally hide the Refresh button when the current tab lacks `refresh` permission.
- Gate the sidebar entry via `anyOf: [hub::models::read, hub::assistants::read, hub::mcp_servers::read]`.

---

### HIGH-2 — Settings: Admin items rendered for all users; no permission filter
**File:** `src-app/ui/src/modules/settings/SettingsPage.tsx:18-50`
**Severity:** HIGH

`SettingsPage` consumes `slots.get('settingsUserPages')` and `slots.get('settingsAdminPages')` and merges them into a single menu (`SettingsPage.tsx:28-50`) without any `evaluatePermission` filter or role check:

```tsx
const userSettingsItems = (slots.get('settingsUserPages') || []).sort(...)
const adminSettingsItems = (slots.get('settingsAdminPages') || []).sort(...)
// ... menuItems is the unfiltered union
```

Concrete impact: a non-admin user sees the "Admin" group divider with admin entries such as `settings/assistants` (template management, `assistants/module.tsx:68-76`) and `settings/hardware` (`hardware/module.tsx:42-50`). The server backstop will 403 the API call, but the UI exposes admin features to non-admins.

Also: when `urlSection` does not match any item in `validSections`, `currentSection` silently falls back to `validSections[0]` (`SettingsPage.tsx:64-66`) — a deep-link to a settings page the user cannot see redirects to the first available, **silently swallowing** the requested URL. There is no inline 403 panel rendered when the user lands on a settings sub-route they don't have permission for; the Outlet (`SettingsPage.tsx:187`) just renders whatever the child route produces, and the child renders the data-or-error from the API.

**Recommendation:** Per plan, filter both `userSettingsItems` and `adminSettingsItems` against `evaluatePermission` for each item's required permission (the `SettingsPageSlot` interface at `types/SettingsSlots.ts:3-9` would need a `permission` or `permissions` field). Render `<Result status="403">` inline at `SettingsPage.tsx:187` (preserve the layout shell, do NOT route-redirect) when the URL section is not in the filtered `validSections`. Preserve URL so the user has a chance to ask an admin for access.

---

### HIGH-3 — Hardware: SSE module-globals defeat StrictMode and break across mounts
**File:** `src-app/ui/src/modules/hardware/Hardware.store.ts:38-43, 107-247`
**Severity:** HIGH (correctness)

The Hardware store uses **module-level mutable state** for SSE management:

```ts
let sseAbortController: AbortController | null = null
let isIntentionallyDisconnecting = false
let isCurrentlyConnecting = false
let lastDisconnectTime = 0
```

Problems:
1. **Two consumers, one global state.** Both `HardwareMonitor.tsx` (in a popup window at `/hardware-monitor`, BlankLayout) and `HardwareSettings.tsx` mount and call `subscribeToHardwareUsage` / `disconnectHardwareUsage`. Both also subscribe in their own `useEffect` and `disconnect` in cleanup (`HardwareMonitor.tsx:25-32`, `HardwareSettings.tsx:36-44`). If the user opens the Hardware settings page, then clicks "Monitor" to open the popup, the popup's mount triggers `subscribeToHardwareUsage` → hits the `sseAbortController !== null` guard and skips (`Hardware.store.ts:118-123`). The popup then **never receives updates** — it sees `currentUsage` from the parent's store *only if it's the same window*; in a real `window.open` popup it's a separate JS context and a separate store entirely, so the popup will silently sit at "Connect to hardware monitoring to view real-time usage data" (`HardwareMonitor.tsx:319-324`). 
   - Confirmed: `window.open(..., 'hardware-monitor', ...)` (`HardwareSettings.tsx:580-584`) opens a separate browsing context; the two pages do NOT share a Zustand store instance.
2. **Settings-page navigation off /settings/hardware** triggers `disconnectHardwareUsage`, which sets `lastDisconnectTime = Date.now()`. If the user navigates back within 200 ms, `subscribeToHardwareUsage` silently skips with a log line (`:108-115`) and the user sees a permanently-disconnected card with no retry.
3. The 200 ms heuristic at `:110` is a band-aid for React StrictMode double-mounting; the real fix is to scope this state inside the store (via `getState/setState`) and use a refcount + an explicit lifecycle, not module-globals. Strict mode produces unmount/remount and `useEffect` runs twice on dev. The current code happens to work in production because StrictMode only double-runs in dev, but the design is fragile.
4. The `subscribeToHardwareUsage` call inside `handleManualConnect` (`HardwareMonitor.tsx:48-50`, `HardwareSettings.tsx:569-575`) does not respect that the auto-mount might still be in the 200 ms window — clicking "Connect" can silently no-op.

**Recommendation:** Replace module-globals with a single `connectionId` counter inside the store; gate connect/disconnect via `getState().sseConnected`/`sseConnecting`; drop the 200 ms timer entirely; make `disconnect` idempotent based on the controller reference inside state, not a module var. For the popup, either (a) post messages from the settings tab to the popup, or (b) accept that the popup runs its own SSE — currently this is broken in both directions.

---

### MED-1 — Settings: `validSections` is a fresh array every render → `useEffect` re-runs forever
**File:** `src-app/ui/src/modules/settings/SettingsPage.tsx:54-78`
**Severity:** MED (perf / potential extra navigations)

`validSections` is derived inline from `menuItems.filter(...).map(...)` (`:54-62`) — a new array reference on every render. It's then listed in the `useEffect` dependency array at `:78`. Because reference equality is broken every render, the effect's "redirect to first section" body checks `location.pathname === '/settings'` each pass; on the *initial* render at `/settings/general`, the check is false so `navigate` isn't called and the loop is benign, but:
- Adding any future logic that calls `navigate` outside the path-check (e.g. validating that `currentSection` is in `validSections`) would create a render loop.
- React's exhaustive-deps lint would flag this.
- The `selectedKeys={[currentSection || validSections[0]]}` (`:105, :162`) re-creates an array literal on every render too, but that's an Antd-render concern only.

**Recommendation:** Wrap `userSettingsItems`, `adminSettingsItems`, `menuItems`, and `validSections` in `useMemo` keyed on `slots`. Same pattern HubPage uses correctly at `HubPage.tsx:22-24`.

---

### MED-2 — Hub submodules: locale hardcoded to 'en'
**File:** `src-app/ui/src/modules/hub/modules/llm-models/stores/hub-models-store.ts:54`, `hub-assistants-store.ts:48`, `hub-mcp-servers-store.ts:48`
**Severity:** MED (consistency / i18n debt)

All three hub stores have the identical pattern:

```ts
const locale = 'en' // TODO: Get from user settings
const models = await ApiClient.Hub.getModels({ lang: locale })
```

Three TODOs that should resolve via `Stores.ConfigClient` (or a future i18n store), but currently every user — regardless of any locale preference saved anywhere — receives English content. Inconsistency with the backend's `lang` parameter being a real per-request input.

**Recommendation:** Extract to a single `Stores.ConfigClient.getLocale()` getter (the store needs the field added) and use it everywhere. Single source of truth.

---

### MED-3 — Hub: `localProvidersLoaded` flag persists in store forever, masks errors
**File:** `src-app/ui/src/modules/hub/modules/llm-models/stores/hub-models-store.ts:92-101`
**Severity:** MED

```ts
loadLocalProviders: async () => {
  const state = get()
  if (state.localProvidersLoaded) return
  try {
    const response = await ApiClient.Hub.getLocalProviders()
    set({ localProviders: response.providers, localProvidersLoaded: true })
  } catch {
    set({ localProvidersLoaded: true })  // ← swallows error, marks as loaded
  }
}
```

If the initial call fails (network blip, server starting up), `localProvidersLoaded` is set to `true` and no subsequent `loadLocalProviders()` call ever retries — yet `localProviders` stays `[]`. The next time the user clicks Download in `ModelHubCard`, the error path fires "No local provider found" (`ModelHubCard.tsx:45-50`) misleading the user into thinking an admin hasn't configured providers when really the call failed.

Also: the error is swallowed entirely — no `set({ error: ... })`, no `console.error`. Silent failure.

**Recommendation:** Either keep retrying (drop the `localProvidersLoaded` guard, or treat it as soft-cache), OR distinguish `localProvidersError` from "empty" and refresh on user action like clicking Download.

---

### MED-4 — Assistants: `loadUserAssistants` short-circuits even when `assistants` is empty after error
**File:** `src-app/ui/src/modules/assistants/stores/UserAssistants.store.ts:111-115`
**Severity:** MED

```ts
loadUserAssistants: async (): Promise<void> => {
  const state = get()
  if (state.isInitialized || state.loading) {
    return
  }
```

After a `catch` block, `loading` is reset to `false` but `isInitialized` is never set; OK so far. But if the catch path runs in `__init__.assistants` (lazy load on first mount), the user sees the empty state with no retry mechanism — `UserAssistantsPage.tsx:212-233` shows "No assistants yet" with a "Create Assistant" button but no "Retry" hint, and the `error` toast (`:30-34`) only flashes once via `clearUserAssistantsStoreError`. The store offers no public retry path because the `isInitialized` guard at `:114` allows only the first attempt to proceed — actually no, the guard is on `isInitialized || loading`, so subsequent calls *should* re-attempt. But the guard does not get cleared on error, and since `isInitialized` only becomes `true` after success, subsequent calls would re-attempt. So this one is actually OK on second look. Removing from MED list.

**Recheck:** The bug is different — `clearUserAssistantsStoreError` is called from a `useEffect([error, message])`, so the moment the toast shows, the error is wiped from state. The user only sees a brief toast and an empty list with no further indication anything failed. That's a poor pattern but not a correctness bug. **Demote to LOW.**

---

### MED-5 — Hub: `useEffect` with `[urlActiveTab, visibleTabs, navigate]` — `visibleTabs` rebuilt every render after slot change
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:22-37`
**Severity:** MED (perf, latent loop)

```tsx
const hubTabs = useMemo(() => {
  return (slots.get('hubTabs') || []).sort((a, b) => a.order - b.order)
}, [slots])
const visibleTabs = hubTabs  // ← same reference

useEffect(() => {
  if (!urlActiveTab && visibleTabs.length > 0) {
    navigate(`/hub/${visibleTabs[0].id}`, { replace: true })
  }
}, [urlActiveTab, visibleTabs, navigate])
```

Today this works (visibleTabs === hubTabs and is memoed against `slots`). But once the permission plan lands and `visibleTabs = hubTabs.filter(t => evaluatePermission(...))`, the filtered array is a new reference every render unless wrapped in `useMemo`. When that lands, the effect will keep firing `navigate(replace=true)` and lose the user's URL. Pre-emptive: must wrap the filter result in `useMemo([hubTabs, user, perms])`.

**Recommendation:** When you wire the filter, memoize `visibleTabs`.

---

### MED-6 — Hardware: inline-style proliferation breaks theming
**File:** `src-app/ui/src/modules/hardware/HardwareSettings.tsx` (~48 inline `style={{...}}` blocks)
**Severity:** MED (consistency / theming)

48 inline-style usages with hardcoded `fontSize`, `padding`, `marginTop`, `marginBottom`, `display: 'flex'`, `flexDirection`, `gap` etc. (see `:62, :63, :115, :154, :173, :253-258, :430, :449-454, …`). Some values inherit from Antd's token system; many do not. In dark mode, hardcoded `style={{ color: ... }}` would break — none observed currently, but the volume signals risk. The pattern is also inconsistent with the rest of the codebase that uses Tailwind utility classes (e.g. the Hub modules use `className="text-xs"`, `flex gap-3`, etc.).

Also: `bg-gray-50` hardcoded in two Drawers:
- `hub/modules/assistants/components/AssistantDetailsDrawer.tsx:41`
- `hub/modules/mcp/components/McpServerDetailsDrawer.tsx:42`

`bg-gray-50` is a literal light-gray and **does not adapt to dark mode**. In dark mode the wrapped instruction text / command-line text will render light-gray-on-dark-background. Confirmed pattern in `ModelDetailsDrawer.tsx` does NOT use `bg-gray-50`; this is inconsistent within hub itself.

**Recommendation:** Replace `bg-gray-50` with a token-aware class or wrap with Antd `Card` token bg. Migrate HardwareSettings inline styles to Tailwind classes for consistency with rest of repo.

---

### MED-7 — Projects module: registered with sidebar entry but the page is a placeholder
**File:** `src-app/ui/src/modules/projects/ProjectsPage.tsx:1-12`, `module.tsx:23-42`
**Severity:** MED (UX/consistency)

`ProjectsPage` is literally:
```tsx
export default function ProjectsPage() {
  return (
    <div className="p-8">
      <Title level={2}>Projects</Title>
      <Paragraph>Projects functionality coming soon...</Paragraph>
    </div>
  )
}
```

Yet the module registers BOTH a `sidebarPrimaryActions` entry "New Project" → `/projects/new` (`module.tsx:24-32`) and a `sidebarNavigation` entry "Projects" → `/projects` (`module.tsx:33-41`). Result:
- The "New Project" CTA in the primary sidebar leads to `/projects/new`, a route that's NOT registered (only `/projects` is). The user gets the 404 fallback.
- Users with no use for "coming soon" placeholders see the link in the sidebar regardless.

Inconsistency with other modules (Hub, Settings, Assistants) which only register slots for fully working pages.

**Recommendation:** Either implement the page or unregister the slot entries until ready. At minimum, remove the `/projects/new` `sidebarPrimaryActions` entry since the route doesn't exist.

---

### MED-8 — Hub Search/Sort: every render rebuilds the `tags` Set and sorts the array
**File:** `src-app/ui/src/modules/hub/modules/llm-models/components/ModelsHubTab.tsx:22-28, 31-62`, identical pattern in `AssistantsHubTab.tsx:23-29, 31-62`, `McpServersHubTab.tsx:22-28, 31-61`
**Severity:** MED (perf)

The `tags` collection and `filtered + sorted` array ARE wrapped in `useMemo` (good). But:
- `useMemo` deps include `[models, searchTerm, selectedTags, sortBy]` — `models` is itself a fresh reference whenever the immer-store yields a new state (which is on every `set`). With Zustand's typical `set(...)` calls, the array reference IS updated only when the slice changes, so this should be fine in practice. Verify via React Profiler if perf becomes an issue.
- The `setSearchTerm(e.target.value)` on every keystroke (`ModelsHubTab.tsx:95`, `AssistantsHubTab.tsx:96`, `McpServersHubTab.tsx:96`) triggers the full filter+sort on every character — debouncing would help if the catalog grows past a few hundred entries.

**Recommendation:** If catalogs exceed ~200 entries, debounce `searchTerm` updates by 150-200 ms. Below that, ignore.

---

### LOW-1 — `ConfigClient.store.ts` has no `__init__`; persisted state lacks explicit hydration probe
**File:** `src-app/ui/src/modules/config-client/ConfigClient.store.ts:26-47`
**Severity:** LOW

The store uses `persist` middleware (good), but does not expose `__init__` or any signal that hydration completed. `ThemeProvider` (`src/components/ThemeProvider/ThemeProvider.tsx:16`) reads `Stores.ConfigClient.themePreference` synchronously. On first mount, Zustand-persist will return `defaultState.themePreference = 'system'` momentarily before localStorage rehydration; this means a flash of system-theme even for users who chose dark/light. Minor cosmetic issue (Antd's `ConfigProvider` swap is fast), but inconsistent with auth/etc. stores that gate UI on `isInitialized`.

**Recommendation:** Add `_hasHydrated` via `onRehydrateStorage` from zustand-persist; have `ThemeProvider` render a small no-op until hydrated. Lowest-priority polish.

---

### LOW-2 — Assistants: `is_default` toggle is a Switch with no exclusivity check
**File:** `src-app/ui/src/modules/assistants/components/AssistantFormDrawer.tsx:227-238`
**Severity:** LOW (UX)

Setting `is_default: true` on one assistant should presumably clear `is_default` from other assistants (or there's a uniqueness constraint server-side). The form UI gives no feedback — no "This will replace your current default" warning. If the backend has no uniqueness constraint, the user can end up with two defaults and `getUserDefaultAssistant` (`UserAssistants.store.ts:252-254`) returns the first one Map-iteration encounters, which is insertion-order — non-obvious.

**Recommendation:** Either enforce uniqueness server-side and surface a confirm dialog ("This will replace 'Foo' as your default"), or document the multi-default behavior. Out of scope to fix without backend coordination.

---

### LOW-3 — Hub `ModelDetailsDrawer.store.ts` and `McpServerDetailsDrawer.store.ts` are registered but never used
**File:** `src-app/ui/src/modules/hub/modules/llm-models/components/ModelDetailsDrawer.store.ts`, `mcp/components/McpServerDetailsDrawer.store.ts`, `mcp/module.tsx:26-29`, `llm-models/module.tsx:24-27`
**Severity:** LOW (dead code)

Both drawer stores expose `isOpen`, `selectedModel`/`selectedServer`, `open`, `close` and are registered in the module's `stores` array. But the drawer components (`ModelDetailsDrawer.tsx`, `McpServerDetailsDrawer.tsx`) receive `model`/`server`/`open`/`onClose` as props from the parent card (`ModelHubCard.tsx:415-419`, `McpServerHubCard.tsx:209-213`), which uses local `useState` (`ModelHubCard.tsx:27`, `McpServerHubCard.tsx:24`). The stores are dead.

Notice also `hub/modules/assistants/module.tsx` does NOT register an `AssistantDetailsDrawer` store — consistent with `AssistantHubCard.tsx` which uses local `useState` (`:22`). The other two carry vestigial registrations.

**Recommendation:** Either route the drawer state through the store (and use `Stores.ModelDetailsDrawer.open(model)` from the card) or drop the store registrations and files.

---

### LOW-4 — Hardware: `gpu_devices` fallback "match by index if only one of each" is unsafe
**File:** `src-app/ui/src/modules/hardware/HardwareSettings.tsx:240-249`
**Severity:** LOW

```ts
const gpuUsage =
  currentUsage?.gpu_devices.find(usage => usage.device_id === gpu.device_id) ||
  (hardwareInfo.gpu_devices.length === 1 &&
   currentUsage?.gpu_devices.length === 1
    ? currentUsage.gpu_devices[0]
    : undefined)
```

The single-GPU fallback masks `device_id` mismatches; if the backend emits an empty/unstable `device_id` for one device and a stable one for another, the wrong card may pair up. Low risk in practice (matching on device_id should be reliable), but the heuristic should at least `console.warn` if invoked.

---

### LOW-5 — `bytes` formatter uses 1024 base but labels "MB", "GB" (should be "MiB", "GiB")
**File:** `src-app/ui/src/modules/hardware/utils/formatBytes.ts:10-16`
**Severity:** LOW (label correctness)

```ts
const k = 1024
const sizes = ['Bytes', 'KB', 'MB', 'GB', ...]
```

Dividing by 1024 and labeling "MB" mixes binary and decimal units (a true "MB" is 1,000,000 bytes; 1,048,576 bytes is "MiB"). User-facing memory readouts on modern systems are almost universally labeled with binary-derived "GB" anyway, so this is the de-facto convention, but it's technically inconsistent. The MED-6 finding also notes this affects display consistency.

**Recommendation:** Optional — change labels to `KiB`/`MiB`/`GiB`. Lowest priority since the rest of the industry has settled on the misuse.

---

### LOW-6 — Hub MCP `created_ids` mutation does not handle re-create-after-uninstall race
**File:** `src-app/ui/src/modules/hub/modules/mcp/stores/hub-mcp-servers-store.ts:115-133`
**Severity:** LOW

The `mcp_server.deleted` event handler filters the deleted id out of `created_ids`. But there's no `mcp_server.created` handler — if a user installs a hub server, deletes it, then installs it again from the same hub card, the hub store relies on `createFromHub` (`:86-113`) to push the new entity_id into `created_ids`. That works for the same card instance, but if some other code path (e.g. user manually creating a server with the same `hub_id` reference) creates a non-hub server, `created_ids` won't reflect it. Not currently exploitable but a latent inconsistency. Same pattern in hub-assistants-store and hub-models-store.

---

### LOW-7 — UserAssistantsPage placeholder grid divs leak hover effects
**File:** `src-app/ui/src/modules/assistants/pages/UserAssistantsPage.tsx:201-204`
**Severity:** LOW (cosmetic)

```tsx
{/* Placeholder divs for grid layout */}
<div className="min-w-70 flex-1"></div>
<div className="min-w-70 flex-1"></div>
<div className="min-w-70 flex-1"></div>
```

The "trailing flex-1 placeholders" hack is a workaround for `flex-wrap` not equalizing card widths on the last row. It works visually but produces 3 empty interactive boundaries. With `aria-hidden="true"` or `inert` they'd be clean; without, screen readers will tab through three empty divs. Replace with CSS grid (`grid grid-cols-[repeat(auto-fill,minmax(17.5rem,1fr))]`) and the placeholders disappear.

---

### LOW-8 — Hub `LazyComponentRenderer` fallback "Loading..." is plain text
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:152`
**Severity:** LOW (UX/consistency)

```tsx
<LazyComponentRenderer
  component={currentTabSlot.component}
  fallback={<div>Loading...</div>}
/>
```

While each `*HubTab` has its own `<Spin size="large" />` block, the brief moment before the lazy chunk arrives shows a bare unstyled "Loading..." string. Compare with the rest of the codebase which uses Antd's `<Spin />`. Minor visual jarr.

---

### LOW-9 — Mobile dropdown header conflict in HubPage
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:113-131`
**Severity:** LOW (responsive)

Mobile (xs) header layout puts the dropdown inside a `flex flex-1 items-center px-2` directly to the right of the title (`:113-114`). The title can be long ("Hub" is short, no actual collision possible), but the `<IoIosArrowForward />` icon between title and dropdown is decorative and gets no `aria-hidden="true"` — screen reader will announce it. Same for `<IoIosArrowDown />` (`:127`). Minor accessibility nit.

---

### LOW-10 — AssistantFormDrawer doesn't reset form when switching between editingAssistant ids without close
**File:** `src-app/ui/src/modules/assistants/components/AssistantFormDrawer.tsx:39-63`
**Severity:** LOW

The `useEffect([open, editingAssistant, form])` re-fills the form when `editingAssistant` changes, but only sets the fields explicitly named. If a future field is added to `editingAssistant` but the effect's `setFieldsValue` payload doesn't include it, stale state will leak. Use `form.resetFields()` then `setFieldsValue` for forward-compat.

---

## Summary table

| Severity | Count |
|---|---|
| HIGH | 3 |
| MED  | 8 |
| LOW  | 10 |

## Cross-references for the executive summary

- **Permission plan: NOT applied** in this branch (HIGH-1, HIGH-2). All hub tabs and admin settings entries are rendered to every user.
- **Hardware SSE design is fragile** (HIGH-3). Module-globals + 200 ms timer to defend against StrictMode is a smell; popup window can silently never receive updates.
- **Theming consistency** (MED-6) — `bg-gray-50` in two drawers; ~48 inline styles in HardwareSettings. Adopt token-aware classes.
- **Stub Projects module** (MED-7) ships sidebar links pointing at `/projects/new` (unregistered) and a placeholder page.
- **Hub locale hardcoded** to `'en'` in three places (MED-2).
- **ConfigClient** hydration is implicit; potential brief flash of default theme on cold load (LOW-1).
- **Dead drawer stores** in Hub LLM-models and MCP submodules (LOW-3).

Files audited (~25):
- `hub/{HubPage.tsx, module.tsx, types.ts, types/HubTabSlot.ts}`
- `hub/modules/llm-models/{module.tsx, stores/hub-models-store.ts, types.ts, components/{ModelHubCard,ModelsHubTab,ModelDetailsDrawer,ModelDetailsDrawer.store}.tsx?}`
- `hub/modules/assistants/{module.tsx, stores/hub-assistants-store.ts, types.ts, components/{AssistantHubCard,AssistantsHubTab,AssistantDetailsDrawer}.tsx}`
- `hub/modules/mcp/{module.tsx, stores/hub-mcp-servers-store.ts, types.ts, components/{McpServerHubCard,McpServersHubTab,McpServerDetailsDrawer,McpServerDetailsDrawer.store}.tsx?}`
- `assistants/{module.tsx, types.ts, pages/{UserAssistantsPage,AssistantsSettings}.tsx, components/{AssistantCard,AssistantFormDrawer,AssistantDrawer.store}.tsx?, stores/{UserAssistants,TemplateAssistants}.store.ts, stores/index.ts, events/{emitters,types}.ts}`
- `settings/{SettingsPage,SettingsLayout,module,index}.tsx?, components/SettingsPageContainer.tsx, types/SettingsSlots.ts`
- `settings-general/{GeneralSettings,module}.tsx, components/ThemeSettings.tsx`
- `hardware/{HardwareMonitor,HardwareSettings,Hardware.store,module,types}.tsx?, utils/formatBytes.ts`
- `projects/{ProjectsPage,module}.tsx`
- `config-client/{ConfigClient.store,module}.tsx?`
