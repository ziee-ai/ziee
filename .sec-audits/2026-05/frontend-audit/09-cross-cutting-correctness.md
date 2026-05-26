# Cross-cutting correctness audit

Scope: every `*.ts`/`*.tsx` under `src-app/ui/src/`, with focus on stores
(`src/modules/*/stores/`, `src/modules/*/*.store.ts`) and async-heavy
React components. Lens = correctness (bugs primary, inefficiencies
spillover). Path style: absolute.

## Summary

- **Bugs found**: 18 (1 HIGH, 12 MED, 5 LOW)
- **Inefficiencies found**: 6
- **Top hotspots**:
  - `Chat.store.ts` — streaming `AbortController` not aborted on
    `__destroy__`; pagination `loadConversations` race when page is
    explicitly passed; `localStorage.getItem('auth-storage')` parsed
    with no try/catch.
  - Race conditions in 6+ `load*` actions when caller passes an
    argument that bypasses the dedup guard (e.g. paginated lists,
    per-id loaders).
  - 4 store-scoped `Stores.EventBus.on(...)` subscriptions with NO
    `__destroy__` cleanup (hub × 3, Auth × 1) — leaks listener slots
    across the proxy's refTracker destroy/re-init cycle.
  - 1 brittle `loading` guard in `SystemMcpServer.loadSystemServers`
    that uses `&&` where `||` is intended — dedup never fires on the
    cold path.
  - SSE: `Hardware.store.ts` and `LlmModelDownload.store.ts` keep
    AbortController in module-scope; if store is destroyed by the
    proxy refTracker, the SSE keeps running and reconnect is blocked.

## Bugs

### B-1 [HIGH] Streaming `AbortController` not aborted on `Chat.store.__destroy__`

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/core/stores/Chat.store.ts:1459-1485`

`Chat.store.__destroy__` clears cache timers and saves conversation
state but does **not** call `state.streamingAbortController?.abort()`.
If the user navigates away from `/chat/:id` mid-stream and the proxy
refTracker schedules destruction (5 s delay, see `core/stores.ts:11`),
the in-flight SSE `fetch` keeps running, the `set(...)` callbacks
appended to it execute against a frozen state, and the destroy log
fires while the network stream is still draining. The store is then
re-initialized on the next visit, which spawns a second concurrent
fetch — leak grows per navigation.

Fix: `if (state.streamingAbortController) state.streamingAbortController.abort()`
near the top of `__destroy__`, mirroring `Hardware.store.ts:228-231`.

### B-2 [MED] `localStorage.getItem('auth-storage')` parsed with no try/catch

`/home/pbya/projects/ziee-chat/src-app/ui/src/api-client/core.ts:10-18`

```ts
export const getAuthToken = () => {
  const authData = localStorage.getItem('auth-storage')
  if (authData) {
    const parsed = JSON.parse(authData)   // ← throws on corrupt JSON
    return parsed.state?.token || null
  }
  return null
}
```

Every API call invokes `getAuthToken()`. If the value gets corrupted
(power loss mid-write, manual user edit, downgrade from a future store
version) the entire API client throws synchronously and the app cannot
recover without a manual `localStorage.clear()`. Compare to
`Chat.store.ts:129-137` (`loadAllPanelSnapshots`) which wraps the
parse in try/catch.

### B-3 [LOW] `SystemMcpServer.loadSystemServers` dedup guard skips first-mount concurrent fetches  *(revised 2026-05-23: downgraded from MED)*

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/mcp/stores/SystemMcpServer.store.ts:154-167`

```ts
if (
  state.systemServersInitialized &&
  state.systemServersLoading &&
  !page
) {
  return
}
```

**Revised:** the guard is actually correct for its intended job (post-init concurrent dedup). On first mount `initialized=false` so it correctly proceeds; once a load completes and a second concurrent call fires post-init (e.g., user clicks Refresh twice), the guard fires correctly. The first-mount concurrent-call case is theoretically uncovered but is prevented in practice by the proxy's `propInitCheck` mechanism (`core/stores.ts:203-209`), which dedups auto-init across multiple consumers.

The optional defensive improvement is `if (state.systemServersLoading && !page) return`, which would also catch the rare imperative concurrent-call case. Worth doing but not a real bug today.

Cross-ref: `06-mcp-sandbox.md` F-3 has the parallel revision.

### B-4 [MED] `loadConversations` race when `page` is passed

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/stores/ChatHistory.store.ts:74-127`

```ts
if (state.loading || state.loadingMore) return
```

Guard is correct for back-to-back clicks without an arg, but
`loadConversations(2)` runs to completion and **replaces** the
in-progress page-1 fetch's eventual `set(...)`. There is no captured
request token to discard stale responses, so the user can end up with
a Map of page-2 conversations even though the UI was supposed to be
showing page-1. AbortController not used.

### B-5 [MED] `loadUsers` race on page change

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/user/stores/Users.store.ts:72-106`

Same pattern as B-4. The dedup guard only handles the initial-load
case (`isInitialized && loading && !page`). Subsequent `loadUsers(3)`
calls while `loadUsers(2)` is in flight both run, and whichever
resolves second wins — even if that response is now stale relative
to the user's clicks. No AbortController, no stale-request token.

### B-6 [MED] `loadModelsForProvider` clobbers state if provider changes mid-flight

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:184-224`

Per-providerId state in `llmModelsLoading[providerId]` correctly
tracks in-flight, but if `loadModelsForProvider(A)` is followed by
`loadModelsForProvider(A)` (same provider, second call from a
different code path) there is no dedup — both run, both `set`, both
clear the `llmModelsLoading[providerId]` flag. Mostly idempotent
(same payload) but two API calls per duplicate trigger.

### B-7 [MED] `loadAssistedGroups` race in `McpServerGroupsAssignmentDrawer`

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:25-46`

```tsx
useEffect(() => {
  if (isOpen && selectedServerId) loadAssignedGroups()
}, [isOpen, selectedServerId])
```

If the user opens the drawer for server A then closes-and-reopens for
server B before A's fetch resolves, the A response can `setAssignedIds`
the wrong list. No request-cancellation, no `current` ref to compare
against `selectedServerId` at resolution time.

### B-8 [MED] Module-global SSE controller leaks across store destroy

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/hardware/Hardware.store.ts:39-42`
`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/llm-provider/stores/LlmModelDownload.store.ts:44`

Both stores hold `sseAbortController` in module scope. The store
proxy in `core/stores.ts:55-159` destroys + re-initializes stores
when their refTracker hits zero (5 s grace) — but neither store has
a `__destroy__` that calls `disconnectHardwareUsage()` /
`disconnectSSE()`. The fetch stream keeps reading the server SSE
forever; on next mount, the connection guards (`sseAbortController
!== null` and `state.sseConnected`) cause `subscribeTo*` to early-
return, so the **store thinks it's disconnected** while bytes keep
flowing. Reproducible by leaving a hardware admin page, waiting >5 s,
and coming back.

Fix: add `__destroy__` that calls the disconnect/cleanup function for
each.

### B-9 [MED] 4 stores subscribe via `EventBus.on` with no `__destroy__`

These stores call `Stores.EventBus.on(eventName, handler, GROUP)` in
`__init__.__store__` but have no `__destroy__` that calls
`Stores.EventBus.removeGroupListeners(GROUP)`:

- `/home/pbya/projects/ziee-chat/src-app/ui/src/modules/auth/Auth.store.ts:167-181` group `'AuthStore'`
- `/home/pbya/projects/ziee-chat/src-app/ui/src/modules/hub/modules/llm-models/stores/hub-models-store.ts:120-135` group `'HubModelsStore'`
- `/home/pbya/projects/ziee-chat/src-app/ui/src/modules/hub/modules/mcp/stores/hub-mcp-servers-store.ts:117-132` group `'HubMcpServersStore'`
- `/home/pbya/projects/ziee-chat/src-app/ui/src/modules/hub/modules/assistants/stores/hub-assistants-store.ts:121-135` group `'HubAssistantsStore'`

The store proxy refTracker (`core/stores.ts:90-118`) schedules
destruction 5 s after the last reference drops, then re-inits on next
access (`core/stores.ts:71-76`). On re-init, `__init__.__store__` runs
again and adds a **second** subscriber for the same group — events
now fire 2× per emit, then 3×, etc. AuthStore is the highest-impact
because `onboarding.user_updated` re-sets user state once per stale
subscriber.

(Compare to `ChatHistory.store.ts:379`, `Model.store.ts:176-178`,
`SystemMcpServer.store.ts:458` which do call `removeGroupListeners`
on destroy.)

### B-10 [MED] `Chat.loadConversation` can clobber post-navigation cache hit

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/core/stores/Chat.store.ts:521-606`

If the user navigates from `/chat/A` → `/chat/B` → `/chat/A` and the
initial `loadConversation(B)` fetch is still in-flight when the user
hits A, the eventual `set({ conversation: B, ... })` will overwrite
the cache-hit-restored A state. The guard at line 525-528 catches
"already loaded" but `loadingConversationId === id` is only checked
against the same id, so A → B → A schedules a real fetch for A
(cache eviction race) while B's fetch still runs.

### B-11 [MED] AppLayout drag listeners leak if component unmounts mid-drag

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:35-119`

`handleMouseDown` attaches `mousemove` and `mouseup` to `document`,
cleaned up on `mouseup`. If the user starts dragging and the
component unmounts (route change forced by an event, e.g. session
expiry) before mouseup, the listeners stay attached forever, holding
the closure-captured `sidebarRef`/`spacerRef`.

(`ResizeHandle.tsx:175-186` has the same pattern.)

Fix: move the listener attach into a `useEffect` that returns a
cleanup, or track `aborted` via an `AbortSignal` passed to
`addEventListener({ signal })` and abort it on unmount.

### B-12 [MED] `useKeyboardShortcuts` re-fires every render when caller inlines deps

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/extensions/keyboard/extension.tsx:155-164`

```ts
export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  useEffect(() => {
    const handler = createKeyboardHandler(shortcuts)
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [shortcuts])
}
```

If a caller passes an inline array literal (the natural API), the
`shortcuts` reference changes on every render. The effect tears
down and reinstalls the keydown listener on every render — fine for
GC, BUT it also means a keystroke landing *during* the unmount/
mount window can be lost. The hook is currently unused in the
codebase (only the global `keyboardExtension.initialize` path is
used), so impact is low — but the API is a trap.

### B-13 [MED] No `__destroy__` cancels SSE controllers in `SandboxEnvironments`

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/code-sandbox/stores/SandboxEnvironments.store.ts:45-50`

`sseControllers: Record<string, AbortController>` is module-scope
and only cleaned in `cleanupSse(flavor)` on `complete`/`failed`/
`evictEnvironment`. If the store is destroyed by refTracker mid-
prefetch, the controllers stay attached, the fetch keeps reading
the SSE, and the next mount cannot re-subscribe (guard at line
122 `if (sseControllers[flavor]) return`).

Same fix shape as B-8 — a `__destroy__` that aborts every controller.

### B-14 [MED] `Chat.store.cacheClearTimers` not cleared if conversation deleted from history

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/core/stores/Chat.store.ts:482-492`

`clearConversationCache(id)` cancels the scheduled timer and removes
the cache entry, but `deleteConversation` in `ChatHistory.store.ts`
does **not** call `clearConversationCache(id)` on the Chat store —
so deleting a conversation while its post-navigation cache-clear
timer is pending leaves an orphan timer. Timer fires later, tries to
clear a non-existent entry (safe), but keeps a `setTimeout` handle
alive in `cacheClearTimers` until `__destroy__`.

### B-15 [LOW] Console statements in non-error paths (171 in `src/modules/`)

171 `console.log`/`console.warn`/`console.error` calls in `src/modules/`
(see Appendix 5). Many are debugging breadcrumbs (`[Chat.store] Cache
hit for conversation: ...`) that ship to production users' devtools.
Build-time strip via `vite-plugin-remove-console` or guarded
`if (import.meta.env.DEV)` would clean this up. Bug-class is "noisy
console + potential PII (conversation IDs)".

### B-16 [LOW] `getServersForGroup` sequential N+1 fetches

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/mcp/stores/SystemMcpServer.store.ts:391-410`

```ts
for (const server of allServers) {
  const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: server.id })
  if (groupIds.includes(groupId)) assignedServers.push(server)
}
```

Awaits each `getServerGroups` sequentially. For N system servers this
is N serial round-trips. `Promise.all(allServers.map(...))` would
parallelize. (Inefficiency, not a correctness bug, but on this list
because it's in a store action visible to the user as latency.)

### B-17 [LOW] `Promise.all` in `ChatHistory.bulkDelete` aborts on first failure

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/stores/ChatHistory.store.ts:200-206`

```ts
await Promise.all(
  Array.from(state.selectedIds).map(id => ApiClient.Conversation.delete({ id })),
)
```

If one delete fails (e.g. 403 on a single conversation the user no
longer owns), the entire Promise.all rejects — but other deletes have
already happened on the server. The state update that follows will
not run, so the UI keeps showing them as undeleted. Should be
`Promise.allSettled` with per-id error surfacing.

### B-18 [LOW] `downloadFile` no try/catch — silent failure on download

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/extensions/file/File.store.ts:684-695`

```ts
downloadFile: async (file: FileEntity) => {
  const response = await ApiClient.File.download({ file_id: file.id })  // ← throws → caller error boundary
  const blob = response instanceof Blob ? response : new Blob([response])
  ...
}
```

If the download fails (network, 403, expired link), the user sees
nothing — only a console error if the caller wraps. The
`document.body.appendChild(a)` / `removeChild(a)` path also leaks
the anchor on throw.

## Inefficiencies (E-N)

### E-1 `loadAllPanelSnapshots` called on every conversation switch

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/core/stores/Chat.store.ts:564-575, 590-598, 1422`

The localStorage parse + filter runs on every cache-hit AND every
cache-miss. Could be memoized in-memory and invalidated only on
`savePanelSnapshotForConversation` / `touchPanelSnapshot`.

### E-2 Hub stores load + version-check sequentially

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/hub/modules/llm-models/stores/hub-models-store.ts:55-56`
(and the analogous `hub-mcp-servers-store.ts`, `hub-assistants-store.ts`)

```ts
const models = await ApiClient.Hub.getModels({ lang: locale })
const versionInfo = await ApiClient.Hub.getModelsVersion()
```

These are independent; `Promise.all` halves load time.

### E-3 `LlmProvider.loadLlmProviders` over-fetches models

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts:139-167`

For every provider, fetches 100 models in parallel, then `find`s the
matching `r.status === 'fulfilled' && r.value.providerId === provider.id`
per provider — O(N²) on the results array. Use a Map keyed by
providerId built once.

### E-4 `LlmProviderGroupWidget` has 30 s cache but no TTL eviction

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/llm-provider/widgets/LLMProviderGroupWidget.store.ts:215-223`

After 30 s the next mount re-fetches, but the stale Map entry stays
keyed in `groupProviders` forever (per group viewed). Low-impact, but
on a large groups list the Map keeps every group ever viewed.

### E-5 `useEffect` re-fires on every `enabledServers` identity change

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/extensions/mcp/components/McpConfigModal.tsx:93-114`

```ts
useEffect(() => {
  if (configModalVisible) {
    enabledServers.forEach(async server => { ... })
  }
}, [configModalVisible, enabledServers.length])
```

Deps array uses `enabledServers.length` (string-stable), but the body
captures `enabledServers` (identity-unstable). Logic happens to be
length-driven so OK in practice — but if a server gets enabled in-
place with the same total count, the effect won't re-fire and the new
server's tools won't load until the modal is reopened.

### E-6 `RecentConversationsWidget` re-renders all 20 items on hover

`/home/pbya/projects/ziee-chat/src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx:19, 73`

`setHoveredId(id)` on every mouseenter triggers a re-render of the
entire list (no memoization on `ConversationCard`-equivalent
sub-component). Each item has a `Popconfirm` mounted — Ant Design's
Popconfirm is non-trivial to render. Move hover state into a
per-item child component to localize.

## Appendix 1: Unhandled awaits

Definition: `await ApiClient.X.foo(...)` not within `try { ... }
catch { ... }` (and not `.catch(...)`d on the call expression).
Listed only the ones whose enclosing scope has no error boundary
above them.

| File:line | API call | Caller context | Try/catch? |
|---|---|---|---|
| `chat/extensions/mcp/components/McpConfigModal.tsx:100` | `McpServerRuntime.listTools` | useEffect → forEach async | YES (try/catch present at 99-110) |
| `chat/extensions/mcp/extension.tsx:688` | `Conversation.getMcpSettings` | onConversationLoad | YES (outer try/catch 686-779) |
| `chat/extensions/mcp/extension.tsx:712` | `McpServerRuntime.listTools` | inside outer try | YES (try/catch 711-722) |
| `chat/extensions/mcp/extension.tsx:784` | `Branch.getPendingApprovals` | inside conditional | YES (try/catch 783-806) |
| `chat/extensions/file/File.store.ts:685` | `File.download` (downloadFile) | top-level fn | **NO** — see B-18 |
| `chat/extensions/file/File.store.ts:213` | `File.upload` | inside async map cb | YES (247-262) |
| `chat/core/stores/Chat.store.ts:1319` | `Conversation.update` | updateConversation | YES (1318-1346) |
| `chat/core/stores/Chat.store.ts:647` | `Branch.activate` | activateBranch | **NO** — direct await, no catch, throws to caller |
| `code-sandbox/stores/SandboxEnvironments.store.ts:114` | `CodeSandbox.startPrefetch` | startPrefetch | **NO** — error propagates; state.progress stuck at no-update |
| `mcp/stores/SystemMcpServer.store.ts:347` | `assignServerToGroups` | inside outer try | YES |
| `mcp/components/common/McpServerDrawer.tsx:252` | `setOAuthConfig` | inside try | YES (271-276) |
| `mcp/components/common/McpServerDrawer.tsx:264` | `deleteOAuthConfig` | inside same try | YES |
| `mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:36` | `getServerGroups` | loadAssignedGroups | YES |
| `mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:54` | `assignServerToGroups` | handleSave | YES |
| `llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx:59` | `LlmRepository.list` | loadRepositories | YES (57-73) |
| `llm-provider/components/llm-models/EditLlmModelDrawer.tsx:54` | `LlmModel.update` | handleSubmit | YES (49-77) |
| `llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx:28` | `LlmModel.create` | handleSubmit | YES (21-60) |
| `api-client/core.ts:12` | `localStorage.getItem` + `JSON.parse` | getAuthToken | **NO** — see B-2 |
| `chat/extensions/mcp/Mcp.store.ts:866` | `Mcp.getDefaults` | loadUserDefaults | YES (864-877) |
| `chat/extensions/mcp/Mcp.store.ts:901` | `Mcp.updateDefaults` | saveUserDefaults | YES (899-918) |
| `chat/extensions/mcp/Mcp.store.ts:429` | `Conversation.updateMcpSettings` | saveConversationConfig | **NO** — propagates uncaught to caller (`onMessageSent`) which catches |
| `chat/extensions/mcp/Mcp.store.ts:1008` | `Mcp.respondToElicitation` | resolveElicitation | YES (1007-1024) |

**Net:** ~3 genuine unhandled awaits with user-visible side effects
(B-2, B-18, sandbox startPrefetch). The rest are wrapped at one
level or another. The "known" cases the prompt flagged are wrapped.

## Appendix 2: load\* race-condition status

Definition: `load*`/`refresh*` action that may run concurrently. "Dedup"
= rejects-when-loading guard. "Cancel" = AbortController. "Stale guard"
= captured request token / version check.

| Store | Action | Dedup? | Cancel? | Stale guard? |
|---|---|---|---|---|
| `chat/core/stores/Chat.store.ts` | `loadConversation` | YES (loadingConversationId) | NO | partial (id check on resolve) |
| `chat/core/stores/Chat.store.ts` | `loadMessages` | NO | NO | NO |
| `chat/core/stores/Chat.store.ts` | `loadBranches` | NO | NO | NO |
| `chat/stores/ChatHistory.store.ts` | `loadConversations` | partial (no-arg only) | NO | NO — **B-4** |
| `chat/stores/ChatHistory.store.ts` | `refreshConversations` | inherits | NO | NO |
| `chat/extensions/assistant/Assistant.store.ts` | `loadAssistants` | YES (length check) | NO | NO |
| `chat/extensions/model/Model.store.ts` | `loadProviders` | NO | NO | NO |
| `chat/extensions/file/File.store.ts` | `loadMessageFile` | YES (loading set) | NO | NO |
| `chat/extensions/file/File.store.ts` | `loadThumbnail` | YES (loading set) | NO | NO |
| `chat/extensions/file/File.store.ts` | `loadPreviewPages` | NO | NO | NO |
| `chat/extensions/file/File.store.ts` | `loadFileTextContent` | YES (loading set + content check) | NO | NO |
| `chat/extensions/file/File.store.ts` | `loadFileBinaryContent` | YES (loading set + content check) | NO | NO |
| `chat/extensions/mcp/Mcp.store.ts` | `loadUserDefaults` | NO | NO | NO |
| `mcp/stores/SystemMcpServer.store.ts` | `loadSystemServers` | **BROKEN — B-3** | NO | NO |
| `mcp/stores/McpServer.store.ts` | `listAccessible*` (loadAccessibleServers) | inherits-pattern | NO | NO |
| `mcp/widgets/GroupSystemMcpServersWidget.store.ts` | `loadServersForGroup` | YES (per-id + 30s cache) | NO | NO |
| `mcp/components/system/McpServerGroupsAssignmentCard.store.ts` | `loadGroupsForServer` | YES (per-id + 30s cache) | NO | NO |
| `llm-provider/stores/LlmProvider.store.ts` | `loadLlmProviders` | YES (isInitialized) | NO | NO |
| `llm-provider/stores/LlmProvider.store.ts` | `loadModelsForProvider` | **NO — B-6** | NO | NO |
| `llm-provider/stores/LlmModelDownload.store.ts` | `loadDownloads` (in __init__) | implicit-on-mount | NO | NO |
| `llm-provider/widgets/LLMProviderGroupWidget.store.ts` | `loadAllProviders` | YES (loading) | NO | NO |
| `llm-provider/widgets/LLMProviderGroupWidget.store.ts` | `loadProvidersForGroup` | YES (per-id + 30s) | NO | NO |
| `llm-provider/components/ProviderGroupAssignmentCard.store.ts` | `loadAllGroups` | YES | NO | NO |
| `llm-provider/components/ProviderGroupAssignmentCard.store.ts` | `loadGroupsForProvider` | YES (per-id + 30s) | NO | NO |
| `llm-repository/stores/LlmRepository.store.ts` | `loadLlmRepositories` | YES (isInitialized) | NO | NO |
| `user/stores/Users.store.ts` | `loadUsers` | partial — **B-5** | NO | NO |
| `user/stores/UserGroups.store.ts` | `loadUserGroups` | inherits-pattern | NO | NO |
| `assistants/stores/UserAssistants.store.ts` | `loadUserAssistants` | YES (isInitialized) | NO | NO |
| `assistants/stores/TemplateAssistants.store.ts` | `loadTemplateAssistants` | YES (isInitialized) | NO | NO |
| `hub/modules/llm-models/stores/hub-models-store.ts` | `loadModels` | YES (loading) | NO | NO |
| `hub/modules/mcp/stores/hub-mcp-servers-store.ts` | `loadServers` | YES (loading) | NO | NO |
| `hub/modules/assistants/stores/hub-assistants-store.ts` | `loadAssistants` | YES (loading) | NO | NO |
| `hardware/Hardware.store.ts` | `loadHardwareInfo` | YES (initialized/loading) | NO | NO |
| `llm-local-runtime/stores/RuntimeVersion.store.ts` | `loadRuntimeVersions` (`list`) | partial | NO | NO |
| `code-sandbox/stores/SandboxEnvironments.store.ts` | `loadEnvironments` | NO | NO | NO |
| `code-sandbox/stores/SandboxEnvironments.store.ts` | `resumeRunningTasks` | implicit (sseControllers guard) | partial | NO |
| `code-sandbox/stores/SandboxResourceLimits.store.ts` | `loadLimits` | NO | NO | NO |
| `auth/Auth.store.ts` | `initAuth` | YES (isLoading) | NO | NO |
| `app/App.store.ts` | `loadSetupStatus` (init) | typical-init | NO | NO |
| `user-llm-providers/UserLlmProviders.store.ts` | `loadUserProviders` | typical | NO | NO |
| `onboarding/stores/Onboarding.store.ts` | `loadOnboarding` | typical | NO | NO |

**Summary:** **No store uses AbortController for load actions.** Most
guard against same-action concurrent calls but only one
(`Chat.loadConversation`) attempts a per-id check that's still racy
in the A→B→A scenario (B-10). Stale-result guards (request tokens)
are zero across the codebase.

## Appendix 3: Event listeners / cleanup status

| File:line | Listener type | Cleanup? |
|---|---|---|
| `components/ThemeProvider/ThemeProvider.tsx:29` | `mediaQuery.addEventListener('change')` | YES (effect return) |
| `modules/layouts/app-layout/AppLayout.tsx:115-116` | `document.addEventListener('mousemove','mouseup')` | partial — only on mouseup; orphan if unmount mid-drag (**B-11**) |
| `modules/layouts/app-layout/AppLayout.tsx:170` | `window.visualViewport.addEventListener('resize')` | YES |
| `modules/layouts/app-layout/components/ResizeHandle.tsx:185-186` | `targetWindow.addEventListener('mousemove','mouseup')` | partial — same shape as AppLayout |
| `modules/chat/extensions/keyboard/extension.tsx:126` | `document.addEventListener('keydown', globalKeyboardHandler)` | YES (extension.cleanup) |
| `modules/chat/extensions/keyboard/extension.tsx:158` | `document.addEventListener('keydown', handler)` | YES (effect return) |
| `api-client/core.ts:183,234,272,277` | `xhr.upload.addEventListener` and `xhr.addEventListener` | implicit (XHR lifetime) |
| `modules/chat/pages/NewChatPage.tsx:15-25` | `Stores.EventBus.on('conversation.created')` | YES (effect return) |
| `modules/chat/stores/ChatHistory.store.ts:311,334,354` | `Stores.EventBus.on(...)` × 3 | YES (`__destroy__` calls `removeGroupListeners('ChatHistory')`) |
| `modules/auth/Auth.store.ts:167` | `Stores.EventBus.on('onboarding.user_updated')` | **NO — B-9** |
| `modules/hub/modules/llm-models/stores/hub-models-store.ts:120` | `Stores.EventBus.on('llm_model.deleted')` | **NO — B-9** |
| `modules/hub/modules/mcp/stores/hub-mcp-servers-store.ts:117` | `Stores.EventBus.on('mcp_server.deleted')` | **NO — B-9** |
| `modules/hub/modules/assistants/stores/hub-assistants-store.ts:121` | `Stores.EventBus.on('assistant.deleted')` | **NO — B-9** |
| `modules/chat/extensions/model/Model.store.ts` (lines 68/77/104/116/133/150 etc) | `EventBus.on(...)` × 6 | YES (`__destroy__` at 176-178) |
| `modules/mcp/stores/SystemMcpServer.store.ts` (multiple via emit/subscribe) | `EventBus.on(...)` × N | YES (`__destroy__` at 458) |
| `modules/llm-provider/stores/LlmProvider.store.ts` (multiple) | `EventBus.on(...)` | YES |

**Net:** 4 stores leak listeners across destroy/re-init cycles
(see B-9). The two drag handlers (B-11) leak only in the unmount-
mid-drag edge case.

## Appendix 4: Event-only widget sweep

Sweep: every component in `**/widgets/**` plus anything with `Widget`
in the file name.

| Component | Subscribes to events? | Mount-time fetch? | Verdict |
|---|---|---|---|
| `chat/widgets/RecentConversationsWidget.tsx` | indirect (ChatHistory store subs) | YES (`!isInitialized → loadConversations()`) | OK |
| `llm-provider/widgets/LLMProviderGroupWidget.tsx` | NO (component) — store subs to events | YES (`useEffect → loadProvidersForGroup`) | **FIXED** per CLAUDE.md |
| `mcp/widgets/GroupSystemMcpServersWidget.tsx` | NO (component) — store subs to events | YES (`useEffect → loadServersForGroup`) | **FIXED** per CLAUDE.md |
| `llm-provider/components/widgets/DownloadIndicatorWidget.tsx` | YES (indirect via `Stores.LlmModelDownload.downloads` reactive) | NO mount-fetch — relies on parent store's SSE | OK (parent SSE always-on) |
| `llm-provider/components/ProviderGroupAssignmentCard.tsx` | indirect | YES (`useEffect → loadGroupsForProvider`) | OK (FIXED per CLAUDE.md history) |
| `mcp/components/system/McpServerGroupsAssignmentCard.tsx` | indirect | YES (`useEffect → loadGroupsForServer`) | OK (FIXED per CLAUDE.md history) |

**No new event-only widgets found.** The previously documented
broken-archetype is fully remediated. `DownloadIndicatorWidget` is
the only one that has no mount-fetch, but its data source
(`LlmModelDownload.store.ts`) is loaded eagerly via `__init__` and
maintained via an SSE stream that opens on first reference, so the
widget legitimately doesn't need a fetch of its own.

## Appendix 5: Console statements

171 `console.{log,warn,error,debug}` lines in `src/modules/`. Top
offenders:

| File | log+warn+error+debug |
|---|---|
| `chat/core/stores/Chat.store.ts` | ~18 (mostly cache + lifecycle breadcrumbs) |
| `chat/extensions/mcp/extension.tsx` | ~16 |
| `chat/extensions/mcp/Mcp.store.ts` | ~7 |
| `llm-provider/stores/LlmModelDownload.store.ts` | ~17 |
| `hardware/Hardware.store.ts` | ~14 (SSE state breadcrumbs) |
| `code-sandbox/stores/SandboxEnvironments.store.ts` | ~6 |
| (rest distributed across modules) | ~93 |

Most are dev breadcrumbs (`[Chat.store] Cache hit for conversation:
abc-123`) that ship to production. Recommend either:
- Gate behind `if (import.meta.env.DEV) console.log(...)`
- Or strip at build time via Vite plugin
- Genuine error paths (`console.error('Failed to ...')`) can stay
  but ideally route through a single error reporter.

---

**Caveat on store-destroy leak severity (B-8, B-9, B-13):** the
proxy's refTracker only destroys a store when **all** reference
counts drop to zero AND no React component holds a subscription. In
practice, many of the leaking stores have at least one always-mounted
consumer (Auth store via `AppShell`, hub stores via the active hub
tab), so the destroy/re-init cycle may rarely fire in the user's
session. The leak is still real and worth fixing, but the user-
visible failure mode is "double events fire after the user closes
all hub tabs for >5 s and reopens one" — niche but reproducible.
