# Frontend Audit — LLM Modules

**Date:** 2026-05-23
**Auditor:** Audit Agent 5 of 9
**Scope:**
- `src-app/ui/src/modules/llm-provider/` (~33 .tsx files, plus stores/events/widgets/icons)
- `src-app/ui/src/modules/llm-repository/`
- `src-app/ui/src/modules/llm-local-runtime/`
- `src-app/ui/src/modules/user-llm-providers/`

**Lens priority:** Bugs → Inconsistencies → Inefficiencies → Responsive
**Method:** Read-only static review against the spec checklist; no edits.

---

## Executive Summary

The LLM frontend stack is functionally mature — admin and user-facing
flows are both implemented, downloads are wired through SSE, store
patterns are uniformly applied, and the previously-known
`LLMProviderGroupWidget` "no-mount-fetch" bug is **already fixed** with
both `useEffect`-mount fetch and event-driven invalidation in place.

However, the **API-key-display surface area is unsafe**: the admin
"Edit Provider" drawer and the "Remote Provider Settings" form both
pre-fill the `api_key` form field directly from `provider.api_key`,
relying on the backend to actually return the cleartext key.
`RemoteProviderSettings` even exposes a **copy-to-clipboard button**
for that key. This is the frontend half of the system-key-leak CRIT-1
issue documented in `2026-05/06-llm-provider.md` (and unchanged from
2025-11). The same pattern affects `LlmRepositoryDrawer` for
`api_key`/`password`/`token` auth credentials.

A few additional medium-impact issues:

- The provider list and per-provider models load **without
  AbortControllers or stale-request guards**: switching providers
  quickly during a slow models fetch can race a stale `set` against
  the new one (`LlmProvider.store.ts:184-224`).
- `LocalProviderSettings`, `LlmProviderSettings`, and
  `LlmModelDownload.store` ship verbose `console.log` debug
  instrumentation in production code paths (21 occurrences inventory).
- `RemoteProviderSettings` and `LlmProviderDrawer` write API keys
  through React form state on every keystroke — a state-leak vector if
  any DevTools/Redux instrumentation is enabled.
- `RuntimeDownloadDrawer` uses `width={500}` while every other LLM
  drawer uses `size={600}` — minor visual divergence.
- `LlmProviderWithModels` custom type extends `BaseLlmProvider` and
  carries TODO comment; backend should canonicalize.

**Severity counts**

| Severity | Count |
|---|---|
| HIGH    | 4 |
| MED     | 9 |
| LOW     | 8 |

**Top-5 risks** (front-end perspective)

1. **F-FE-LLM-01 (HIGH).** Admin edit drawer pre-fills `api_key` into
   visible (`<Input.Password>`) form field; same in
   `RemoteProviderSettings`. Frontend assumes the backend ships
   cleartext keys — and it does (per `06-llm-provider.md` CRIT-1).
   Frontend has no defensive mask, no "(unchanged)" sentinel, no
   diff-on-save behavior.
2. **F-FE-LLM-02 (HIGH).** `RemoteProviderSettings.tsx:154-163` adds
   a literal copy-to-clipboard button for the system API key.
   Trivial credential exfiltration via shoulder-surfing or any
   clipboard-monitor extension.
3. **F-FE-LLM-03 (HIGH).** `LlmRepositoryDrawer.tsx:32-37` pre-fills
   `auth_config.api_key`, `password`, and `token` from the repository
   object into the edit form. Same root cause: backend echoes the
   secret back, frontend trusts it.
4. **F-FE-LLM-04 (HIGH).** `loadModelsForProvider` race —
   `LlmProvider.store.ts:184-224` has no AbortController and no
   stale-result guard. Rapid provider-switch can apply old models
   over new state.
5. **F-FE-LLM-05 (MED).** Production `console.log` debug spam (21+
   occurrences across `LlmProvider.store`, `LlmModelDownload.store`,
   `LocalProviderSettings`, `LlmProviderSettings`). Leaks provider
   IDs / download IDs to console; minor info disclosure + perf.

---

## Findings

---

### F-FE-LLM-01 — Admin "Edit Provider" drawer pre-fills cleartext API key into the form (HIGH)

* **Severity:** HIGH
* **Lens:** Bug + Inconsistency
* **Files:**
  * `src-app/ui/src/modules/llm-provider/components/LlmProviderDrawer.tsx:33-48`
  * `src-app/ui/src/modules/llm-provider/components/RemoteProviderSettings.tsx:93-103, 118-122`

**Behavior**

```tsx
// LlmProviderDrawer.tsx:33-48
useEffect(() => {
  if (provider && open) {
    form.setFieldsValue({
      name: provider.name,
      provider_type: provider.provider_type,
      api_key: provider.api_key,        // ← cleartext from backend
      base_url: provider.base_url,
      enabled: provider.enabled,
    })
  } ...
}, [provider, open, form])
```

`provider.api_key` arrives in plaintext from `/api/llm-providers/list`
(see `.sec-audits/2026-05/06-llm-provider.md` F-01). Toggling the
eye-icon on the `<Input.Password>` then reveals it in cleartext. A
shared-screen demo or a screen-recorded admin walkthrough now spills
the system API key.

**Why this is a frontend bug even though the root cause is backend**

The frontend has options the backend doesn't:

1. **Don't write the existing key into the form at all.** Use a
   sentinel like `KEY_DISPLAY_PLACEHOLDER` (the user-LLM-providers
   page already does this — see
   `UserLlmProvidersPage.tsx:25, 47-50` — `'••••••••••••••••••••••••'`).
2. **Send the key field only when changed.** On submit, if
   `values.api_key === KEY_DISPLAY_PLACEHOLDER || values.api_key ===
   provider.api_key`, drop the field from the PATCH payload (don't
   round-trip the secret).
3. **Never put an eye-toggle on existing keys.** Allow toggle only
   when the user is entering a *new* key in this session.

The admin drawer currently does none of these.

**Recommendation**

Mirror the `UserLlmProvidersPage` pattern in `LlmProviderDrawer` and
`RemoteProviderSettings`. Render a `••••` placeholder when
`provider.api_key` is set, clear it on focus, and only PATCH
`api_key` when the field is non-empty AND non-placeholder.

---

### F-FE-LLM-02 — Copy-to-clipboard for system API key (HIGH)

* **Severity:** HIGH
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/RemoteProviderSettings.tsx:36-43, 154-163`

**Behavior**

```tsx
// RemoteProviderSettings.tsx:154-163
suffix={
  <Button
    type="text"
    icon={<CopyOutlined aria-hidden="true" />}
    onClick={() => copyToClipboard(currentProvider.api_key || '')}
    aria-label="Copy API key to clipboard"
  />
}
```

The "Copy API key" button reads `currentProvider.api_key` directly
(not the password-masked input element). Any admin browsing the
provider page can hit Copy and now has the system credential in the
OS clipboard — accessible to clipboard managers, screen-share tools,
malicious browser extensions, etc.

**Recommendation**

Remove the copy-to-clipboard affordance entirely from the system-key
view. If a copy is genuinely needed, gate it behind a fresh re-auth
challenge and a single-shot "show key" reveal modal with audit
logging.

---

### F-FE-LLM-03 — Repository drawer pre-fills auth secrets into form (HIGH)

* **Severity:** HIGH
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-repository/components/LlmRepositoryDrawer.tsx:26-45, 47-105`

**Behavior**

```tsx
// LlmRepositoryDrawer.tsx:27-38
if (repository && open) {
  form.setFieldsValue({
    name: repository.name,
    url: repository.url,
    auth_type: repository.auth_type,
    api_key: repository.auth_config?.api_key,      // ← cleartext
    username: repository.auth_config?.username,
    password: repository.auth_config?.password,    // ← cleartext
    token: repository.auth_config?.token,          // ← cleartext
    ...
  })
}
```

`auth_config` is the same shape as the LLM provider api_key: the
backend echoes cleartext credentials, and the drawer cheerfully
sticks them into reveal-able `Input.Password` fields.

The "Test Connection" button (`testRepositoryFromForm`,
`LlmRepositoryDrawer.tsx:47-105`) reads `form.getFieldsValue()` and
POSTs the full credential set to a test endpoint — even when the
admin merely opened the drawer to look at the name and never
touched the credentials. That re-transmits the secret on the wire
on the round-trip.

**Recommendation**

Same as F-FE-LLM-01: render `••••` placeholder, only PATCH a field
when changed in-session, and don't include unchanged secrets in the
test-connection payload (or test against the stored credentials by
ID, server-side).

---

### F-FE-LLM-04 — `loadModelsForProvider` has no stale-request guard (HIGH)

* **Severity:** HIGH
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:184-224`

**Behavior**

```ts
// LlmProvider.store.ts:184-224
loadModelsForProvider: async (providerId: string) => {
  try {
    set(state => ({ llmModelsLoading: { ..., [providerId]: true } }))
    const modelsResponse = await ApiClient.LlmModel.list({ providerId, ... })
    set(state => ({
      providers: state.providers.map(p =>
        p.id === providerId
          ? { ...p, llm_models: modelsResponse.models }
          : p,
      ),
      ...
    }))
  } ...
}
```

No `AbortController`, no in-flight dedup. Two concurrent calls for
the same provider race: the second `set` always wins; whichever
request resolves last writes the final state. In a slow-network
scenario, switching providers (which can fire `loadModelsForProvider`
from multiple places — `LlmProviderSettings`, `LocalProviderSettings`
mount, the download SSE `update` handler at
`LlmModelDownload.store.ts:233-235`, and the `complete` handler at
`LlmModelDownload.store.ts:276-280`) can apply an old response over
a fresh one.

The download-SSE path is the most pathological: every SSE `update`
event with a `completed` status fires
`loadModelsForProvider(providerId)` (line 233-236), so a burst of
SSE updates can saturate the API and reorder responses.

Contrast with `loadLlmProviders` at line 116-181 which **does** have
an `isInitialized || loading` early-return guard.

**Recommendation**

```ts
// Track per-provider in-flight controllers
const inFlight: Map<string, AbortController> = new Map()

loadModelsForProvider: async (providerId: string) => {
  inFlight.get(providerId)?.abort()
  const ctrl = new AbortController()
  inFlight.set(providerId, ctrl)
  try {
    const response = await ApiClient.LlmModel.list(
      { providerId, ... },
      { signal: ctrl.signal },
    )
    if (ctrl.signal.aborted) return
    set(state => ({ ... }))
  } finally {
    if (inFlight.get(providerId) === ctrl) inFlight.delete(providerId)
  }
}
```

---

### F-FE-LLM-05 — `console.log` debug instrumentation in production paths (MED)

* **Severity:** MED
* **Lens:** Inefficiency + Info disclosure
* **Files:**
  * `src-app/ui/src/modules/llm-provider/stores/LlmModelDownload.store.ts:102-106, 185-198, 211-216, 228-235, 259-275, 282-283, 295-296, 304-308, 320-321, 331, 362-364, 376` (multiple)
  * `src-app/ui/src/modules/llm-provider/components/LlmProviderSettings.tsx:129-138`
  * `src-app/ui/src/modules/llm-provider/components/LocalProviderSettings.tsx:16`

**Behavior**

21+ `console.log` statements remain in shipped LLM-module code,
including:

- Provider IDs and provider_type on every settings render
  (`LlmProviderSettings.tsx:129, 133, 138`)
- Download IDs and provider IDs on every SSE update
  (`LlmModelDownload.store.ts:228-235, 272-275`)
- SSE state-machine narration ("SSE connected:", "SSE update:", "SSE
  complete:", "Disconnecting SSE...", "Reconnection attempt N/5", …)

These are:

* Information-disclosure risk for users with browser DevTools open
  (provider/download IDs are not super sensitive but they help an
  attacker correlate requests).
* Performance noise (SSE update narration on a fast download fires
  many times per second).
* Console clutter that hides real warnings.

**Recommendation**

Strip all `console.log` from production builds — either remove them,
or gate behind `if (import.meta.env.DEV)`. Keep `console.error` for
genuine error paths.

---

### F-FE-LLM-06 — `LlmProviderDrawer` updates `provider_type` field but it's locked on edit; create payload still inherits stale defaults (MED)

* **Severity:** MED
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/LlmProviderDrawer.tsx:33-77`

**Behavior**

When opening for *create*, the drawer pre-sets `provider_type: 'local'`
and `enabled: true`. When opening for *edit*, it sets the full form.

Edge case: the user opens the drawer for editing provider X (a remote
provider), closes it, then immediately clicks "Add Provider" — `form`
state retains the previous provider's field values because the
`useEffect` only sets defaults when `!provider && open`, and
`form.resetFields()` is called only in `handleClose`. If the drawer
re-opens via the route or some other code path that doesn't run
`handleClose`, the stale values persist.

`PROVIDER_TYPES` is also locally hard-coded
(`LlmProviderDrawer.tsx:13-23`) duplicating the icon map in
`constants.tsx:7-17`. Adding a new provider requires editing both.

**Recommendation**

Derive `PROVIDER_TYPES` from a single source-of-truth that the
backend's `provider_type` enum points to (or generates from OpenAPI).
Call `form.resetFields()` in the open-as-create branch of the
useEffect for safety.

---

### F-FE-LLM-07 — Provider `enabled` toggle UI relies on permissive helper (`llmProviderHasCredentials` always returns true) (MED)

* **Severity:** MED
* **Lens:** Inconsistency
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:355-361`

**Behavior**

```ts
llmProviderHasCredentials: (_provider) => {
  // API key is no longer required to enable a provider.
  // Users can supply their own keys via their profile settings.
  return true
}
```

But `ProviderHeader.tsx:46-59` still calls this and computes
`canEnableProvider`/`getEnableDisabledReason` from it — and the
returned `'API key is required for remote providers'` string is
unreachable (because the helper always returns true).

The dead-code branch is misleading: a reader expects the toggle to
disable when no key is set, but it never does.

**Recommendation**

Either:
- Remove `llmProviderHasCredentials` and the dead helpers entirely.
- Or implement real credential detection (e.g., check `provider.api_key !== ''`
  AND `provider.api_key_configured`).

Same applies to `LlmProvider.store.ts:294-300` (`updateLlmProvider`
return value preservation) — code path is fine but coupled to the
above helper's truthiness.

---

### F-FE-LLM-08 — `loadLlmProviders` parallel-loads models for every provider on startup (MED)

* **Severity:** MED
* **Lens:** Inefficiency
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:138-167`

**Behavior**

`loadLlmProviders` does a parallel fan-out: for each provider (up
to 50), it fires `ApiClient.LlmModel.list({ providerId, page:1,
per_page: 100 })`. So on app boot, the user can trigger 50
concurrent requests just by being logged in to a deployment with
many providers.

For most deployments this is fine (3-10 providers). For a managed
multi-tenant Ziee with all 9 default + custom providers, this is 9+
parallel calls just to populate the providers list.

The `loadLlmProviderGroupWidgetStore.loadAllProviders` at
`LLMProviderGroupWidget.store.ts:154-197` does *another* providers
list call (with `per_page: 1000`), so the same data is fetched
twice during boot.

**Recommendation**

Backend should return providers with eager-loaded `llm_models`
(matches the existing `LlmProviderWithModels` TODO comment at
`LlmProvider.store.ts:23-27`). Failing that, lazy-load models when
the user actually navigates to a provider, not for *all* providers
on app boot.

The `LlmProviderGroupWidget.store` cache also overlaps the main
`LlmProvider.store`; consolidate.

---

### F-FE-LLM-09 — `loadAllProviders` per_page=1000 magic number; no pagination UI (MED)

* **Severity:** MED
* **Lens:** Inefficiency
* **File:** `src-app/ui/src/modules/llm-provider/widgets/LLMProviderGroupWidget.store.ts:172-176`

**Behavior**

`per_page: 1000` to "guarantee" loading everything. If a deployment
ever exceeds 1000 providers (unlikely but conceivable for a managed
SaaS), the UI silently truncates. No pagination controls anywhere
in the LLM-provider list (`LlmProviderSettings.tsx:64-78` builds the
menu off `providers` directly with no pagination).

Same magic-number per_page=1000 appears in
`ProviderGroupAssignmentCard.store.ts:125-128` for groups.

**Recommendation**

Either accept the cap and add a "showing first 1000 of N — refine your
search" UX, OR add proper pagination/virtualization. For a settings
sidebar with 50+ entries, a search/filter input would be a better UX.

---

### F-FE-LLM-10 — `LlmProviderWithModels` custom type lingers — TODO unresolved (MED)

* **Severity:** MED
* **Lens:** Inconsistency
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:23-27`

```ts
// Extended type that includes models array
// TODO: Backend should include llm_models in LlmProvider response
export interface LlmProviderWithModels extends BaseLlmProvider {
  llm_models?: LlmModel[]
}
```

Two issues:

1. The TODO has been open long enough that the backend never adopted
   it; consequently every component that accepts a provider has to
   know about this custom type, and the cast happens at multiple
   creation/update return sites (`createLlmProvider:248-254`,
   `updateLlmProvider:294-299`).
2. The backend already exposes `ProviderWithModels` for the user-facing
   `/user-llm-providers` endpoint (`UserLlmProviders.store.ts:7`). The
   admin and user types diverge.

**Recommendation**

Either eliminate the custom type and read models from the dedicated
`Stores.LlmModel` store, OR formalize `ProviderWithModels` as the
canonical type returned by both admin and user endpoints.

---

### F-FE-LLM-11 — Drawer width inconsistency: `RuntimeDownloadDrawer` (500) vs. all other LLM drawers (600) (MED)

* **Severity:** MED
* **Lens:** Responsive + Inconsistency
* **File:** `src-app/ui/src/modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:50`

8 LLM drawers use `size={600}`. One uses `width={500}`:

```
LlmProviderDrawer.tsx:97                            size={600}
LlmProviderGroupsAssignmentDrawer.tsx:110           size={600}
GroupLlmProvidersAssignmentDrawer.tsx:79            size={600}
AddRemoteLlmModelDrawer.tsx:86                      size={600}
EditLlmModelDrawer.tsx:103                          size={600}
AddLocalLlmModelDownloadDrawer.tsx:251              size={600}
AddLocalLlmModelUploadDrawer.tsx:391                size={600}
LlmRepositoryDrawer.tsx:193                         size={600}
RuntimeDownloadDrawer.tsx:50                        width={500}   ← outlier
```

Different prop too (`width` vs `size`) — the layout `Drawer` wrapper
accepts `size` (typed in `app-layout/components/Drawer`). The
Runtime drawer is calling Ant's raw `Drawer` directly and bypassing
the project wrapper.

**Recommendation**

Use the project's `<Drawer>` wrapper from
`@/modules/layouts/app-layout/components/Drawer`, and standardize to
`size={600}` for visual cohesion.

---

### F-FE-LLM-12 — Drawer footer pattern divergence: half use `footer={null}` + inline buttons, half pass an array (MED)

* **Severity:** MED
* **Lens:** Inconsistency
* **Files:**
  * `LlmProviderDrawer.tsx:96, 172-179` — `footer={null}` + inline div
  * `LlmRepositoryDrawer.tsx:192, 367-381` — same pattern
  * `LlmProviderGroupsAssignmentDrawer.tsx:111-126` — `footer={<div>…</div>}`
  * `GroupLlmProvidersAssignmentDrawer.tsx:80-94` — `footer={<div>…</div>}`
  * `AddRemoteLlmModelDrawer.tsx:73-85` — `footer={[<Button />, <Button />]}`
  * `EditLlmModelDrawer.tsx:90-102` — `footer={[<Button />, <Button />]}`
  * `AddLocalLlmModelDownloadDrawer.tsx:207-250` — `footer={…conditional array}`
  * `AddLocalLlmModelUploadDrawer.tsx:377-390` — `footer={[<Button />, <Button />]}`

Five different footer-rendering patterns across 8 drawers. Buttons
are styled inconsistently (some `flex justify-end gap-2`, some
`flex justify-end gap-3 pt-4`, some `<Space>`). Submission states
also vary — most use `loading={loading}`, but `LlmProviderDrawer`
uses `loading={loading || creating || updating}` while
`LlmRepositoryDrawer` uses the same. The semantic difference
(disable Cancel during save vs not) is also inconsistent.

**Recommendation**

Extract a `<DrawerFooter primaryLabel="Save" onPrimary={…}
onCancel={…} saving={…} />` component used by all 8 drawers.

---

### F-FE-LLM-13 — `RemoteProviderSettings` form re-binds form fields on every `currentProvider` reference change (MED)

* **Severity:** MED
* **Lens:** Inefficiency
* **File:** `src-app/ui/src/modules/llm-provider/components/RemoteProviderSettings.tsx:93-103`

```tsx
useEffect(() => {
  if (currentProvider) {
    form.setFieldsValue({ api_key: currentProvider.api_key, base_url: ... })
    setHasUnsavedChanges(false)
    setPendingSettings(null)
  }
}, [currentProvider, form])
```

`currentProvider` is computed via `Stores.LlmProvider.providers.find(...)`
on every render — a new object reference if any store change
triggers a re-render, even if the provider data itself didn't
change. So the effect re-runs on *every* store update, calling
`setFieldsValue` and silently wiping `hasUnsavedChanges` /
`pendingSettings`.

If the user is mid-edit and any event fires (e.g., another tab
modifies a model, an SSE download update lands), their unsaved
changes vanish.

**Recommendation**

Use `useMemo` to stabilize `currentProvider` or depend on
`currentProvider.id` instead of the object reference:

```ts
}, [currentProvider?.id, form])
```

Same pattern issue exists at `LlmProviderDrawer.tsx:33-48`.

---

### F-FE-LLM-14 — SSE reconnection backoff is constant 3s for 5 attempts then gives up (MED)

* **Severity:** MED
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmModelDownload.store.ts:300-326`

```ts
const attempts = state.reconnectAttempts + 1
const maxAttempts = 5
if (attempts < maxAttempts) {
  setTimeout(() => { void get().subscribeToDownloadProgress() }, 3000)
}
```

Constant 3-second retry × 5 attempts = SSE permanently dead after 15
seconds of network blip. No exponential backoff, no manual retry
button, and the UI's only signal is `sseError` text. After max
attempts, downloads silently stop receiving progress updates — they
*are* still running on the backend, but the UI never reflects it
until next refresh.

**Recommendation**

- Exponential backoff (3s, 6s, 12s, 24s, 60s) with jitter.
- Surface a "Reconnect" button in the download indicator widget.
- Periodically re-poll the downloads list as a fallback if SSE is
  permanently dead.

---

### F-FE-LLM-15 — `ViewDownloadDrawer` doubles as `AddLocalLlmModelDownloadDrawer`; overloaded form state (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx:29-200`

The same drawer component handles two modes:

* `addMode` — opening from `AddLocalLlmModelDownloadDrawer.open`
* `viewMode` — opening from `ViewDownloadDrawer.open`

`const open = viewMode || addMode` (line 33). Form is reused via
`disabled={viewMode}` (line 319). This couples two different
workflows (creating a download vs. viewing an in-flight one); the
"Cancel Download" button at line 217-235 only appears in viewMode.

This works but it's confusing — two stores for two modes of the
same drawer, and the close handler `handleCloseModal` closes both
stores (line 77-82) so opening the wrong one always succeeds in
closing.

**Recommendation**

Split into two separate components; share read-only form
rendering through a sub-component. The cancel-download action
should live with `LlmModelDownload.store` actions, not buried
in the view drawer.

---

### F-FE-LLM-16 — `AddLocalLlmModelDownloadDrawer` ships a hardcoded default model name (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx:189-198`

```tsx
form.setFieldsValue({
  display_name: 'TinyLlama Chat Model',
  description: 'Small 1.1B parameter chat model for quick testing (~637MB)',
  file_format: 'safetensors',
  repository_path: 'meta-llama/Llama-3.1-8B-Instruct',
  main_filename: 'model.safetensors',
  repository_branch: 'main',
})
```

The defaults are mismatched (`TinyLlama` display name but
`Llama-3.1-8B-Instruct` repo path). This is dev/test scaffolding
that shipped.

**Recommendation**

Remove dev defaults — start with empty fields or use a
"templates" dropdown ("Try Llama 3.1 8B", "Try TinyLlama", etc.).

---

### F-FE-LLM-17 — `LlmModelDownload.store` `setupDownloadTracking` uses module-scoped boolean flag (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmModelDownload.store.ts:46, 345-373`

```ts
let isSubscriptionSetup = false  // module-scoped

setupDownloadTracking: (): void => {
  if (isSubscriptionSetup) return
  isSubscriptionSetup = true
  useLlmModelDownloadStore.subscribe( ... )
}
```

`isSubscriptionSetup` lives at module scope, never reset. If the
store is ever torn down and re-created (HMR, future logout-relog
flow), the subscription is lost forever because the flag
remembers it as "set up." Same for `sseAbortController` on line 44.

**Recommendation**

Move the flag into store state, and reset it in `__destroy__` if a
cleanup is ever added. Currently no `__destroy__` exists on this
store — the SSE connection survives logout.

---

### F-FE-LLM-18 — `LlmProviderSettings` re-renders entire menu on every store change due to inline `menuItems` calculation (LOW)

* **Severity:** LOW
* **Lens:** Inefficiency
* **File:** `src-app/ui/src/modules/llm-provider/components/LlmProviderSettings.tsx:64-85`

```tsx
const menuItems = providers.map(provider => {
  const IconComponent = PROVIDER_ICONS[provider.provider_type] || ...
  return { key: provider.id, label: <Flex>...</Flex> }
})
menuItems.push({ key: 'add-provider', ... })
```

`menuItems` rebuilds on every render. Both the desktop
`<Menu />` and the mobile `<Dropdown />` get fresh array references,
forcing Ant to re-diff. For 50 providers, this is measurable on
older laptops.

**Recommendation**

`useMemo` the `menuItems` array against `providers`.

---

### F-FE-LLM-19 — `LlmModelsSection` "disable provider when last model disabled" UX is jarring (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/LlmModelsSection.tsx:49-87`

When a user disables the last enabled model, the provider is
automatically disabled too — but the toggle visibly flips with a
delay (one API call, then another). The message says "Model
disabled. Provider disabled as no models remain active." but doesn't
ask for confirmation.

If the user wanted to disable just this model temporarily, they now
have a disabled provider too and have to manually re-enable it
(possibly losing track of the original state).

**Recommendation**

- Use `App.modal.confirm` to ask first.
- Or move the cascading logic to the backend (semantically more
  correct, since it's a business rule).

---

### F-FE-LLM-20 — `LlmProvider.store.ts` parallel `Promise.allSettled` for model loading swallows errors silently (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:139-167`

The `loadLlmProviders` parallel model fetch catches all errors per
provider and falls back to `models: []`. So if the user has 5
providers and one fetch fails (permission, network, server error),
that provider silently shows as having "0 models" — no warning, no
retry, no diagnosis. The error goes only to `console.error`.

`modelError` state exists but is *only* populated by the per-provider
`loadModelsForProvider` path (line 220-222), not the bulk-load path.

**Recommendation**

Populate `state.modelError[providerId]` on per-provider failures in
the parallel path too, and surface it in `LlmModelsSection`.

---

### F-FE-LLM-21 — Per-model `LlmModelsSection` `useParams` reads `providerId` non-defensively in a non-route mount context (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/components/LlmModelsSection.tsx:25-38`

```tsx
const { providerId } = useParams<{ providerId?: string }>()
const loading = llmModelsLoading?.[providerId!] || false
//                                       ^^^^^^^^^^ non-null assertion
```

If the component is ever rendered outside the
`/settings/llm-providers/:providerId` route, `providerId` is
`undefined` and `llmModelsLoading[undefined!]` becomes an actual
`llmModelsLoading['undefined']` lookup. Not a security bug — just
brittle.

Same pattern in `ProviderHeader.tsx:28-33` and
`ProviderGroupAssignmentCard.tsx:14-22` (the latter is more careful
— bails on `!providerId`).

**Recommendation**

Bail early if `!providerId`. Don't use `!` non-null assertion.

---

### F-FE-LLM-22 — User-LLM-providers page lacks "Test" button to verify a saved key works (LOW)

* **Severity:** LOW
* **Lens:** Inefficiency
* **File:** `src-app/ui/src/modules/user-llm-providers/UserLlmProvidersPage.tsx:155-187`

The admin RemoteProviderSettings has effectively no test button
either (the proxy form has none; the API key form has only Save),
but the repository drawer does. Users saving their own API key get
no feedback that it actually works — they save, get "saved"
confirmation, then discover it's wrong only when chat fails.

**Recommendation**

Add a "Test Key" button on the user page that runs a trivial
chat-completion against the provider and reports success/failure.

---

### F-FE-LLM-23 — `loadLlmRepositories` early-return on `isInitialized` makes repository list permanently stale (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-repository/stores/LlmRepository.store.ts:71-75`

```ts
loadLlmRepositories: async () => {
  const state = get()
  if (state.isInitialized || state.loading) {
    return  // ← never re-fetches
  }
  ...
}
```

Once loaded, the store never re-fetches even if data may be stale.
The event handlers (`llm_repository.created`/`updated`/`deleted` at
line 298-336) keep the local list in sync for *this* tab's mutations,
but a second tab's mutations are invisible until a hard refresh.
Same pattern in `LlmProvider.store.ts:117-120`.

**Recommendation**

Add an explicit `refetch` action, or accept that the events keep
state coherent enough for single-user scenarios.

---

### F-FE-LLM-24 — `RuntimeUpdateChecker` shows "Click to check for updates" perpetually even when nothing has been clicked (LOW)

* **Severity:** LOW
* **Lens:** Inconsistency
* **File:** `src-app/ui/src/modules/llm-local-runtime/components/RuntimeUpdateChecker.tsx:39-45`

The widget shows an info alert until the user clicks "Check for
Updates" — no auto-check on mount. Compare to the same UX for LLM
model downloads where the SSE auto-subscribes.

**Recommendation**

Either auto-check on mount with a 1-hour TTL, or remove the manual
button and rely on a scheduled background poll.

---

### F-FE-LLM-25 — `RuntimeDownloadDrawer` bypasses the project Drawer wrapper, breaking the layout's responsive behavior (LOW)

* **Severity:** LOW
* **Lens:** Responsive
* **File:** `src-app/ui/src/modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:2, 46-50`

```tsx
import { Drawer } from 'antd'   // raw Ant Drawer
...
<Drawer title=... width={500}>
```

Every other LLM drawer imports `from
'@/modules/layouts/app-layout/components/Drawer'`. The wrapper is
where mobile-responsive sizing and the project's drawer-stack
behavior live. The Runtime drawer skips it, so on a 320px-wide
mobile viewport it renders at 500px (overflows).

**Recommendation**

Switch to the wrapper component, change `width={500}` to
`size={600}` for consistency.

---

### F-FE-LLM-26 — `cancelLlmModelDownload` mutates state *before* awaiting the API response (LOW)

* **Severity:** LOW
* **Lens:** Bug
* **File:** `src-app/ui/src/modules/llm-provider/stores/LlmModelDownload.store.ts:122-137`

Actually the code is OK — `await` is first, then `set` removes from
state. But the comment says "Remove from local state immediately
(backend will send update via SSE)" — which is misleading; the
removal happens *after* the cancel API succeeds. If the API
*fails*, the download stays in state, which is correct behavior, but
the comment misrepresents it.

**Recommendation**

Fix the misleading comment, or actually move the optimistic remove
*before* the API call for snappier UX (with rollback on error).

---

## Cross-cutting Observations

### Permission gating

None of the LLM-module components import or render `<Can>` or
`usePermission` guards. The settings sidebar item at
`module.tsx:140-148` is registered as `settingsAdminPages`, so the
admin-only routing is the only access control. Per the audit plan
this is OK to flag rather than treat as a bug — the permission plan
(`llm_providers::*`, `llm_models::*`, `llm_repositories::*`) is
assumed in scope but not applied here.

When the plan is applied:

- All Create/Edit/Delete buttons in `LlmProviderSettings`,
  `ProviderHeader`, `LlmModelsSection`, `LlmRepositorySettings`,
  `LlmRepositoryDrawer` need `<Can permission="llm_providers::manage">` etc.
- Group-assignment widgets (`LLMProviderGroupWidget`,
  `ProviderGroupAssignmentCard`) need
  `<Can permission="llm_providers::manage">`.
- The user-LLM-providers page is user-owned — no admin permission
  required; the backend already enforces user-scoped access.

### Event coverage

Events emitted look complete:

```
llm_provider.{created,updated,deleted,groups_changed,group_providers_changed}
llm_model.{enabled,disabled,deleted}
llm_repository.{created,updated,deleted}
runtime_version.{created,deleted,default_changed}
```

Missing:

- No `llm_model.created` event — `AddRemoteLlmModelDrawer` calls
  `addLlmModelToProvider` directly (line 48) and reloads
  (`loadLlmProviders` line 49), bypassing the event system. So
  another store listening for "model created" can't react.
- No `llm_model.updated` event — `EditLlmModelDrawer` calls
  `updateLlmModelInProvider` directly (line 64-68).
- No `llm_model.upload_completed` or `download_completed` events —
  these are surfaced through SSE update handlers only.

### Drawer width / footer / form-binding archetype

Most drawers follow a similar pattern but with subtle divergence:

| Drawer | `size`/`width` | Footer pattern | `maskClosable` |
|---|---|---|---|
| LlmProviderDrawer | 600 | inline div | false |
| LlmRepositoryDrawer | 600 | inline div | false |
| AddRemoteLlmModelDrawer | 600 | array of buttons | false |
| EditLlmModelDrawer | 600 | array of buttons | false |
| AddLocalLlmModelUploadDrawer | 600 | array, conditional `closable` | conditional |
| AddLocalLlmModelDownloadDrawer | 600 | conditional array | false |
| LlmProviderGroupsAssignmentDrawer | 600 | inline div | (default) |
| GroupLlmProvidersAssignmentDrawer | 600 | inline div | (default) |
| RuntimeDownloadDrawer | **500** | `<Space>` | (default, raw Ant Drawer) |

Recommend: define a `<LlmDrawer>` thin wrapper with project defaults.

### Race conditions inventory (per checklist)

| Store action | AbortController? | Stale guard? | Dedup? | Verdict |
|---|---|---|---|---|
| `loadLlmProviders` | no | `isInitialized\|\|loading` | yes | OK |
| `loadModelsForProvider` | **no** | **no** | **no** | **BUG (F-FE-LLM-04)** |
| `createLlmProvider` | n/a | `state.creating` | yes | OK |
| `updateLlmProvider` | n/a | `state.updating` | yes | OK |
| `deleteLlmProvider` | n/a | `state.deleting` | yes | OK |
| `enableLlmModel` / `disableLlmModel` / `deleteLlmModel` | n/a | per-model `llmModelOperations[id]` | weak | OK-ish |
| `loadAllProviders` (Widget) | no | `providersInitialized` | yes | OK |
| `loadProvidersForGroup` | no | `existing?.loading` + 30s TTL | yes | OK |
| `loadGroupsForProvider` (Card) | no | `existing?.loading` + 30s TTL | yes | OK |
| `loadLlmRepositories` | no | `isInitialized\|\|loading` | yes | OK |
| `loadVersions` | no | (none) | no | weak — but called from `__init__` only |
| `checkForUpdates` | no | `checking[engine]` | yes | OK |
| `downloadVersion` | n/a | (none) | no | acceptable |

### Responsive

The pages handle mobile-vs-desktop split via
`useWindowMinSize().sm` (LlmProviderSettings, UserLlmProvidersPage).
This works. `RuntimeDownloadDrawer`'s raw Ant Drawer breaks this for
the runtime versions page (see F-FE-LLM-25).

No explicit Tailwind responsive classes (`sm:`, `md:`, `lg:`) used
in any LLM file inspected — relies entirely on Ant's defaults +
`useWindowMinSize`. That's fine in principle.

---

## Suggested remediation order

1. **F-FE-LLM-01 / 02 / 03** — credential exposure cleanup. Three
   files (`LlmProviderDrawer.tsx`, `RemoteProviderSettings.tsx`,
   `LlmRepositoryDrawer.tsx`). Mirror the
   `UserLlmProvidersPage` placeholder pattern.
2. **F-FE-LLM-04** — `loadModelsForProvider` race guard.
3. **F-FE-LLM-05** — strip `console.log` debug.
4. **F-FE-LLM-08 / 10** — backend `ProviderWithModels` adoption (or
   lazy model-loading).
5. **F-FE-LLM-13** — `currentProvider.id` dependency in useEffect
   (preserves unsaved changes).
6. **F-FE-LLM-11 / 25** — Runtime drawer wrapper + width.
7. The remaining LOW items can be batched into a polish pass.

---

**End of Audit Agent 5 report.**
