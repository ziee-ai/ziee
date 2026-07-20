# frontend-perf — Lazy Store Loading: architecture research & design options

Design backing for **ITEM-3** ("split the eager module-discovery so admin/
settings-only modules are not baked into the entry chunk"). This document is
research + a recommended design, **not** an implementation. It answers: *why is
every store in the entry chunk today, which stores can safely become lazy, what
are the mechanisms (with ecosystem prior art), and what breaks if we do it
wrong.*

Baseline entry chunk after ITEM-1: **482.1 KB gzip** (BASELINE.md). The eager
store graph is a major contributor: ~90 registered stores + their transitive
import graphs (`ApiClient` method modules, `api-client/types`, permission
helpers) all land in the entry chunk.

---

## 1. The eager-instantiation chain (mechanism, verified)

Every store is in the entry chunk because of a **static import chain from an
eagerly-globbed module manifest**, not because anything reads it at boot:

1. `src/modules/loader.ts:81` globs **eagerly**:
   `import.meta.glob('./**/module.tsx', { eager: true })` (+ a second eager glob
   for `../components/**/module.tsx`). Every `module.tsx` is therefore in the
   entry graph. This is *intended* — a module's manifest declares its routes,
   slots, permissions and nav entries, which the shell needs at boot to build
   the router table and sidebar.

2. Each `module.tsx` **statically imports its store file(s)** at top level.
   Verified samples:
   - `modules/citations/module.tsx:6` → `import { useCitationsStore } from './stores/Citations.store'`
   - `modules/web-search/module.tsx:7-8` → `WebSearchAdmin.store`, `WebSearchUserKeys.store`
   - `modules/scheduler/module.tsx:10-12` → 3 stores
   - `modules/memory/module.tsx:6-10` → 5 stores

3. The store file **creates the Zustand store at import-evaluation time**. E.g.
   `WebSearchAdmin.store.ts:12` calls `defineStore('WebSearchAdmin', {...})`,
   which runs `create<...>()(applyMiddleware(...))` synchronously
   (`sdk/packages/framework/src/store-kit.ts:196`). So importing the file both
   instantiates the store **and** drags in its entire transitive graph
   (`ApiClient.WebSearch.*`, `api-client/types`, `hasPermissionNow`, …).

4. `createModule` (`module.ts:22`) wraps the already-created handles into
   `registerStores: () => options.stores!`. `registerModule`
   (`store.ts:121-126`) calls `createStoreProxy(reg.store)` and puts the proxy in
   `stores[reg.name]`. The `Stores` proxy (`stores.ts:324-330`) is a **synchronous**
   registry lookup: `useModuleSystemStore.getState().stores[prop]` — a missing
   name returns `undefined`.

**Key finding — the runtime is ALREADY lazy; only the *code* is eager.**
`createStoreProxy` runs the store's `__init__.__store__()` (which wires the
`sync:<entity>` subscriptions + does the initial load) **on first property
access**, not at registration (`stores.ts:239-244`). And the ref-count tracker
tears the store down 5 s after its last reader unmounts
(`stores.ts:130-158`, `DEFAULT_DESTROY_DELAY_MS`). So an eagerly-registered but
never-accessed store **never subscribes to SSE and never fetches** — it is inert
code sitting in the entry chunk. Making such a store lazy therefore changes
**zero runtime semantics**; it only moves bytes out of the entry chunk. This is
the single most important fact for the risk assessment below.

**What actually tethers a store to the entry chunk:** the static `import` in
`module.tsx` (step 2) **and** any barrel that statically re-exports it (see §5).
Cross-module *reads* via `Stores.X` do **not** tether — the proxy lookup is a
runtime string key, not a static import. (Confirmed: BASELINE B4 shows
`Chat.store` is pulled eager only by *in-module* files — `ChatRightPanel`,
`ChatPaneContext`, `chatBridge` — never by the `projects` module that reads
`Stores.Chat`.)

---

## 2. What must stay eager, and why

A store MUST be registered before the shell first renders **iff something
mounted at boot reads it**. The always-mounted surfaces are: the `router` /
`app` / `layouts/app-layout` modules, the `AuthGuard` route guard (wraps every
protected route), `sidebarContent` / `sidebarBottom` / `appBanners` /
`routerEffects` slot components, and every `shouldMount:` gate expression in a
`module.tsx` (they run in the AppShell render on every route —
`AppShell.tsx:84-88` → `ConditionalComponent` → `registration.shouldMount()`).

Two sub-classes of eager:
- **Boot/shell stores** — read directly by boot UI. Genuinely eager.
- **Gate stores** — a *tiny* store whose `.open` flag a `shouldMount` reads
  every render to decide whether to mount a drawer (e.g.
  `scheduler/module.tsx:67` reads `Stores.SchedulerDrawer.open`). The gate store
  is 36 lines (`SchedulerDrawer.store.ts`) — negligible weight — but the drawer's
  **content** stores (which the drawer's lazy chunk pulls) are heavy and can move
  behind the drawer's own lazy boundary (§4, Tier 2).

---

## 3. Ecosystem prior art (transferable mechanisms)

| Source | Mechanism | What transfers here |
|---|---|---|
| **Redux Toolkit `combineSlices` + `inject()`** ([docs](https://redux-toolkit.js.org/api/combineSlices)) | Reducers injected after store init; `withLazyLoadedSlices<T>()` declares the future slices via **declaration merging** so types exist before the code does; a `selector` **Proxy returns the slice's initial state** until it is injected, so a read before injection is safe (never `undefined`). | The declaration-merge half is **already how this codebase types stores** (`RegisteredStores` interface, `import './types'`). The "safe read before registration" via an initial-state stub is the mitigation for cross-module early reads (§4 Tier 3). |
| **redux-dynamic-modules `DynamicModuleLoader`** ([repo](https://github.com/microsoft/redux-dynamic-modules)) | A component declares the module it needs; the module (reducers+middleware) is **added to the store on mount, removed on unmount**. | Direct analog of our recommendation: register a store **from the lazy page chunk that consumes it**, on load. Our ref-count destroy already mirrors the unmount-removal half. |
| **React Router v6.4+/v7 `route.lazy`** ([Remix blog](https://remix.run/blog/lazy-loading-routes)) | Route's non-match parts (loader/Component/…) resolve via an async import; granular v7 splits loader vs Component into separate chunks. | Our routes are *already* lazy (`lazyWithPreload`). The store just needs to ride the **same** lazy boundary as its page instead of being hoisted out by the manifest's static import. |
| **Zustand feature-store splitting** ([discussion #937](https://github.com/pmndrs/zustand/discussions/937)) | Prefer **multiple independent stores**, load on demand via dynamic `import()`. | Validates per-store granularity (we already have ~90 stores) and dynamic-import loading. |
| **Jotai `atomWithLazy` / atomFamily / async atoms** ([lazy](https://jotai.org/docs/utilities/lazy), [family](https://jotai.org/docs/utilities/family)) | Initial value computed at first use; Suspense-integrated async atoms. | The "initialize at first use" model — which our proxy *already does* via `__init__` on first access. Reinforces that deferring the *code* is the only remaining win. |
| **TanStack Query** ([client-state guide](https://tanstack.com/query/v5/docs/framework/react/guides/does-this-replace-client-state)) | Server-state lives in an on-demand cache, not a global store; truly-global client state ends up "very tiny". | Strategic framing: most of these ~90 stores are thin server-state caches (load-on-mount + `sync:` refetch). They are the *ideal* lazy candidates — nothing global depends on them between visits. |

**Bundler mechanics** ([rollup #5627](https://github.com/rollup/rollup/issues/5627),
Vite manualChunks): a module reached by **any** static import lands in the
importer's chunk; a **barrel re-export** that statically re-exports a module
defeats a sibling's dynamic `import()` of it (the exact
`INEFFECTIVE_DYNAMIC_IMPORT` failures in BASELINE B4). So lazy stores require
**removing both static tethers** (the manifest import *and* the barrel).

---

## 4. Design options

### Option A — Co-locate registration in the lazy page chunk (RECOMMENDED for isolated stores)

Drop the store from `module.tsx`'s `stores:` array and its static import; instead
the store self-registers as a **side effect of the lazy page chunk** loading. Two
sub-forms:

- **A1 (side-effect register):** the store file calls a new
  `registerStore(handle)` module-system action at top level; the lazy page
  imports the store file, so evaluating the page chunk registers the store
  **synchronously before the page renders**. `module.tsx` keeps only the
  type-only `import './types'` (erased — free) so `Stores.X` stays typed.
- **A2 (declarative lazy loader):** `module.tsx` declares
  `lazyStores: [{ name: 'X', load: () => import('./stores/X.store').then(m => m.X.store) }]`;
  the router triggers `load()` when it enters a route owned by that module (or
  the page's route element triggers it). Cleaner/declarative, but needs a
  registry + router change and an async gap.

For a store that is **ISOLATED + has no always-mounted consumer**, A1 is ideal:
the only reader lives in the same chunk that registers it, so the synchronous
`Stores.X` proxy **never observes `undefined`** — there is no ordering hazard.
Requires: (1) a `registerStore` action on `useModuleSystemStore` (a few lines —
mirror the `newStores[reg.name] = createStoreProxy(reg.store)` path already in
`store.ts:123`), and (2) removing the barrel re-exports (§5 / ITEM-4).

**Runtime-semantics delta: none** — per §1, an unaccessed store never inits, so
deferring its *code* to page-load is behavior-preserving.

### Option B — Async-tolerant registry with an initial-state stub (for cross-module early reads)

For the "middle bucket" (cross-module readers but no boot reader — e.g.
`projects` reads `Stores.Chat`), A1 has an ordering hazard: reader chunk X may
load before owner chunk Y. Port RTK's `selector`-proxy idea: `Stores.X` returns a
**stub backed by `config.state`** (initial state, no actions wired) and kicks off
`load()`; when the real store arrives, swap it in and let the existing
sync-subscription drive a refetch. This is the only option that makes
cross-module stores lazy **without** an eager handle, but it is the most invasive
(the proxy becomes async/stub-aware) and risks a flash of initial state. **Defer
unless the entry budget still misses target after Tiers 1–2.**

### Option C — Keep a tiny eager handle, lazy-load only the heavy body

Register a minimal eager store (state shape + a `load()` that dynamic-imports the
heavy action/graph on first call). Splits the *transitive* weight (ApiClient
methods, etc.) out of the entry chunk while keeping `Stores.X` synchronously
present for any early reader. Good middle ground for a cross-module store whose
early-read safety matters but whose bulk is in its actions. More boilerplate per
store than A.

### Non-option — bundler `manualChunks` alone

Cannot fix this: as long as `module.tsx` statically imports the store, no
`manualChunks` rule moves it out of the eagerly-imported manifest's reachability.
The static tether must be cut in source first (then Rollup splits it for free).

---

## 5. Hard prerequisite: kill the barrel re-exports (ITEM-4)

Every lazy-store approach is **blocked** until the static barrel re-exports are
removed — otherwise the store is pulled back into the entry graph regardless of
`module.tsx`. From BASELINE B4, the offending barrels that statically re-export
stores:
- per-module `stores/index.ts` (defeats `AssistantPicker.store`,
  `McpComposer.store`, `ModelPicker.store`, `kbSelectionKey`, …)
- `sdk/packages/framework/src/index.ts` (defeats `stores.ts`)
- `src/api-client/index.ts`, `framework/src/api-client/core.ts` (drag the whole
  ApiClient into stores that import it via the barrel)

ITEM-4 and this work are the same dependency edge: **do ITEM-4 first (or jointly)
per store touched.** Cross-module readers must reach the store only through the
runtime `Stores.X` proxy (dynamic) or a module public barrel that does **not**
statically re-export the store *file*.

---

## 6. Store safety classification

Derived from a full `Stores.<Name>` cross-reference sweep (owning `module.tsx`
`stores:` array vs external readers vs always-mounted consumers). "(gate)" = the
store is read only by a `shouldMount` gate; "(gated-internal)" = read only from a
`shouldMount`-gated drawer's chunk (executes on open).

### Tier 1 — LAZY-SAFE now (ISOLATED + no always-mounted consumer) — ~85 stores

Mostly admin/settings/secondary pages. Purely bundle wins, zero runtime-semantics
change. Move to Option A1 + drop the barrel re-export.

`AuthProviders, SessionSettings, Bootstrap, MemorySetupStep, JsToolSettings,
SandboxRootfsVersions, SandboxResourceLimits, FileVersions, Deliverables,
FileRagAdmin, AuthProvidersAdmin, VoiceRuntimeVersion, VoiceUpdate, VoiceModel,
VoiceModelUpdate, VoiceModelUpload, VoiceModelDownloadProgress,
VoiceDownloadProgress, VoiceUploadModelDrawer, VoiceConfig, VoiceInstance,
HubSkills, HubWorkflows, McpServerDetailsDrawer, ModelDetailsDrawer,
GroupSystemMcpServersWidget, SystemMcpServerGroupCard, McpToolCalls,
ProjectMcpSettings, SystemWorkflow, WorkflowRun, WorkflowRuns, WorkflowDrawer,
RuntimeVersion, RuntimeUpdate, RuntimeDownloadDrawer, RuntimeDeleteConfirm,
RuntimeConfig, RuntimeModelUsage, RuntimeDownloadProgress, TemplateAssistants,
AssistantDrawer, KnowledgeBases, KnowledgeBaseDetail, KnowledgeBaseComposer,
Citations, ConversationSkills, SystemSkill, SkillDrawer, SkillConversationDrawer,
Projects, ProjectDrawer, Profile, Users, UserGroups, EditUserGroupDrawer,
GroupMembersDrawer, UserGroupsDrawer, AssignGroupDrawer, CreateUserDrawer,
EditUserDrawer, ResetPasswordDrawer, WebSearchAdmin, WebSearchUserKeys,
UserLlmProviders, UserProviderKeys, Memories, MemorySettings, MemoryAdmin,
MemoryAudit, CoreMemoryBlocks, LitSearchAdmin, LitSearchUserKeys,
LlmProviderDrawer, AddLocalLlmModelDownloadDrawer, AddLocalLlmModelUploadDrawer,
AddRemoteLlmModelDrawer, EditLlmModelDrawer, ViewDownloadDrawer, LlmModelUpload,
ProviderGroupAssignmentCard, LLMProviderGroupWidget, SummarizationAdmin,
ConversationSummarization, Hardware, SchedulerAdmin.`

These map onto ITEM-3's named modules (user, auth-providers, memory, file-rag,
voice, web-search, literature, summarization, scheduler-admin, hub sub-pages,
llm-local-runtime, knowledge-base, citations, hardware) and go further.

### Tier 2 — Gated-drawer internals (medium risk, high value)

Read only from a `shouldMount`-gated drawer chunk; the drawer already lazy-mounts
via `LazyComponentRenderer`. Keep the **tiny gate store** eager
(`SchedulerDrawer`, `FilePreviewDrawer`, `LlmRepositoryDrawer`,
`GroupSystem*Assignment`, `GroupLlmProvidersAssignment`), move the **heavy body**
behind the drawer's own lazy chunk (register on open):

`ScheduledTasks, Workflow, AssistantPicker, McpServer, ModelPicker,
SystemMcpServer, LlmProvider, LlmRepository, GroupSystemWorkflowsWidget,
GroupSystemSkillsWidget.`

Caveat: several of these are also read cross-module (`Workflow`,
`AssistantPicker`, `ModelPicker`, `McpServer` from `scheduler`; `LlmProvider`
cross-refs) — so they straddle Tier 2/3 and need the early-read guard of Option
B/C if a non-drawer reader can precede the drawer.

### Tier 3 — Cross-module, no boot reader (harder; needs Option B/C)

Lazy-capable only via a stub/async registry (early-read ordering hazard):

`Chat, MessageViewState, SplitView, ApiKeysStep, McpServersStep, SandboxFlavors,
File, ProjectFiles, PdfHighlight, HubCatalog, HubInstalled, HubMcpServers,
HubAssistants, McpServerDrawer, McpComposer, McpUserPolicy, UserAssistants, Skill,
ProjectDetail.`

Low payoff for `Chat`/`File` (needed on the primary chat route anyway).
Recommend leaving eager unless the budget still misses.

### Never — boot/shell stores (must stay eager)

`ModuleSystem, Routes, AppLayout, Auth, App, AppMode, ConfigClient, EventBus,
Onboarding, ServerUpdate, Notifications, ChatHistory, HubModels, LlmModelDownload.`
(Read by the sidebar widgets / `AuthGuard` / router / theme provider / boot
`initialize` — `RouterComponent.tsx`, `LeftSidebar.tsx:68-69`,
`DownloadIndicatorWidget.tsx` (sidebarBottom), `LlmModelDownloadNotifications.tsx`
(registered component), etc.)

Note: `UserProfile` and singular `Notification` are **not** real stores (the
sidebar footer reads `Stores.Auth`; the notification store is `Stores.Notifications`).

---

## 7. Recommendation & sequencing

1. **ITEM-4 first (per store touched)** — remove the barrel static re-exports for
   any store being made lazy; otherwise the split is a no-op (BASELINE B4).
2. **Tier 1 via Option A1** — add a `registerStore` action to
   `useModuleSystemStore`; convert the ~85 isolated stores to self-register from
   their (already-lazy) page chunk; delete the `module.tsx` static import +
   `stores:` entry, keep `import './types'`. Zero runtime-semantics change (§1).
   This realizes ITEM-3 and is the bulk of the win.
3. **Tier 2** — split gated-drawer heavy bodies behind the drawer's lazy chunk,
   keeping the gate store eager.
4. **Tier 3 / Option B** — only if the entry-chunk budget (ITEM-8) still misses
   after 2–3; it's the invasive one.

### Guardrails (make regressions impossible)
- **Type safety survives laziness**: `RegisteredStores` declaration merging via
  the erased `import './types'` keeps `Stores.X` typed even though the code is
  lazy (same idea as RTK `withLazyLoadedSlices`). Keep it.
- **Desktop parity (PLAN_AUDIT ITEM-3 CONCERN)**: `loader.desktop.ts` +
  `CORE_MODULE_BLOCKLIST` must still fully suppress a blocked module — with A1 a
  blocked module's page never loads, so its store never registers (consistent),
  but verify no eager path (a barrel, a cross-module static import) imports a
  blocked module's store file on desktop. Diff `loader.desktop.ts` before
  shipping.
- **Early-read safety net**: the `Stores` proxy returning `undefined` for an
  unregistered name is a latent crash for Tier 2/3. Add a dev-mode warning in the
  `Stores` get trap (`stores.ts:325-329`) when a known-but-unregistered name is
  read, so an ordering mistake surfaces loudly in dev instead of as a prod
  `Cannot destructure undefined`.
- **Budget fence**: ITEM-8's entry-gzip + login-chunk-count check is what keeps
  the win from silently regressing when a future `module.tsx` re-adds a static
  store import.

### Risks / open questions
- **Cross-module readers of a Tier-1 store**: the classification says these ~85
  are ISOLATED, but any *future* cross-module reader that imports the store
  **file** (not the `Stores.X` proxy) would re-tether it. The dev-mode warning +
  a lint against importing another module's `*.store.ts` guards this.
- **`sync:<entity>` freshness while away from the page**: unchanged from today —
  an unreferenced store is already torn down after 5 s and re-inits (fresh load)
  on next access (§1). No background-freshness regression, because there is no
  background freshness today.
- **A2 vs A1**: A2 (declarative `lazyStores` + router-triggered load) is the
  tidier long-term API but adds an async gap and a router change; A1 is
  lower-risk and sufficient for Tier 1. Recommend A1 now, consider A2 as a later
  consolidation.
