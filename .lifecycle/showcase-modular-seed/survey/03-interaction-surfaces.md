# Survey 03 — The three hand-listed interaction-only surface classes

Scope: the gallery's three STATIC, hand-authored surface classes — `overlays`,
`deep`, `seeded`. (`pages` is auto-enumerated from the router store and out of
scope.) All paths are under `src-app/ui/src/dev/gallery/`.

Files read in full:
- `overlays.tsx` (OVERLAY_ENTRIES)
- `deepStates.tsx` (DEEP_STATE_ENTRIES)
- `seededSurfaces.tsx` (SEEDED_SURFACE_ENTRIES) + `seeded/helpers.tsx` + `seeded/shard1..5.tsx`
- `interactions.ts` (interaction manifest)
- Skimmed: `DefectRepro.tsx`, `MessageListLongDemo.tsx`, `TableDemos.tsx`

Absolute paths:
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/overlays.tsx`
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/deepStates.tsx`
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/interactions.ts`
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/seededSurfaces.tsx`
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/seeded/helpers.tsx`
- `/data/pbya/ziee/tmp/showcase-seed-wt/src-app/ui/src/dev/gallery/seeded/shard{1..5}.tsx`

Totals: **44 overlays**, **17 deep-states**, **94 seeded** (52 integrator-owned +
10 shard1 + 10 shard2 + 8 shard3 + 8 shard4 + 6 shard5) = **155 hand-listed
interaction surfaces**.

---

## 1. The three entry-shape interfaces (verbatim)

### `OverlayEntry` (`overlays.tsx:25-39`)

```ts
export interface OverlayEntry {
  /** Gallery slug → `?surface=<slug>&state=open`; also the section testid. */
  slug: string
  /** Coverage surface id (the component file). */
  surface: string
  /** Human title for the frame. */
  title: string
  component: LazyExoticComponent<ComponentType>
  /** Seed + fire the store open action (runs on mount). Optional: prop-driven
   *  overlays render open via bound props (see `lazyBound`) with no store call. */
  open?: () => void | Promise<void>
  /** Interaction recipes driven after the overlay opens (focus an input, submit
   *  invalid, …). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
}
```

Notes: `surface` (the component-file id) is UNIQUE to overlays — it feeds the
coverage gate `WIRED_OVERLAY_SURFACES = new Set(OVERLAY_ENTRIES.map(o => o.surface))`
(`overlays.tsx:737`). Neither deep nor seeded carry a `surface` field.

### `DeepStateEntry` (`deepStates.tsx:43-58`)

```ts
export interface DeepStateEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** Which conversation the ConversationPage is pinned to. */
  conversationId: string
  /** One-line note about what deep state this exercises. */
  note: string
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
  /** Interaction recipes: drive real user actions after mount to render the
   *  interaction-gated states (click-to-edit, expand, hover) this surface hides
   *  behind a click. Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
}
```

Notes: deep entries carry **no `component`** — the frame ALWAYS renders the same
lazy `ConversationPage` inside a `MemoryRouter` pinned to `conversationId`
(`deepStates.tsx:39-41`, `DeepStateFrame` `:459-505`). What varies per entry is
`conversationId` + `setup()`. So the deep class is chat-module-only by
construction.

### `SeededSurfaceEntry` (`seeded/helpers.tsx:24-48`)

```ts
export interface SeededSurfaceEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. Keep it UNIQUE
   *  and shard-prefixed (e.g. `seeded-s3-...`) so shards never collide. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** One-line note about the seeded state this reaches. */
  note: string
  /** Route path the component is mounted under (for useParams/useNavigate). */
  path: string
  /** Concrete initial path (params filled). */
  initialPath: string
  /** The real component to render. */
  component: LazyExoticComponent<ComponentType>
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
  /** Interaction recipes driven after the seeded surface mounts (click-to-edit
   *  inline forms, expand). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
  /** Render at natural height instead of the fixed 720px overflow-hidden frame. */
  fullHeight?: boolean
}
```

Notes: seeded is the most general shape — it carries its OWN `component` +
router `path`/`initialPath` (so it can shadow an enumerated route or mount a
prop-driven leaf component). Deep is effectively a specialization of seeded that
hard-codes `component = ConversationPage` and `path = /chat/:conversationId`.

### `InteractionRecipe` (`interactions.ts:45-53`) — shared by all three

```ts
export interface InteractionRecipe {
  /** Recipe name → `?interact=<name>` + screenshot suffix `slug__<name>.png`. */
  name: string
  note?: string
  /** Drive real user actions once the surface has mounted + settled. */
  steps: (d: PageDriver) => Promise<void>
}
```

`PageDriver` (`interactions.ts:25-42`) = a pure-DOM driver (`click`/`type`/`focus`/
`hover`/`waitFor`/`waitForAny`/`wait`/`query`), addressed by `data-testid`, queried
against the WHOLE document (overlays portal to `<body>`). The single in-page driver
is `makeDomDriver()`; `useRunInteraction(interactions, settleMs)` reads
`?interact=<name>` off the URL and drives it once on mount, with a StrictMode
re-entry lock (`interactions.ts:172-193`). `buildInteractionManifest(entries)`
flattens `{slug,name,note}` across any entry-list that has `.interactions`.

---

## 2. How each class SEEDS its state (the key mechanic)

There is NO mock-API cassette override per entry — every class renders the REAL
component/store and lets the gallery's loaded cassette answer GETs; the entry's
job is only to layer the *transient* state a GET-only harness can't reach. Three
seeding channels are used, often combined:

1. **Store setState directly** (the dominant channel). `Store.store.setState({…} as any)`
   on the real Zustand store, wrapped in one of three durability helpers from
   `seeded/helpers.tsx`:
   - `holdPatch(apply, times=26, gap=185)` — re-applies the patch ~26× over
     ~4.8 s so a late-arriving cassette load can't clobber the seeded branch.
   - `holdForever(apply)` — applies now + on a permanent 150 ms `setInterval`
     (reclaimed on navigation); used for LOADING arms whose lazy chunk mounts at
     an unpredictable time.
   - `whenTrue(pred)` — polls until the store's own load finished, THEN patches
     (used when the seed must layer on top of a loaded value, e.g. flip `.error`
     after settings load).
   Overlays' skill seeding uses the same `holdPatch` (imported from `seeded/helpers`).

2. **Store OPEN action** (overlays only, the `open?()` field). Calls the real
   drawer/dialog store's open action — e.g. `Stores.LlmProviderDrawer.openLlmProviderDrawer(provider)`,
   `Stores.WorkflowDrawer.open(workflowFixture)`, or the imperative
   `dialog.info(...)` / `dialog.warning(...)` for the DialogHost singleton. The
   Base-UI Sheet/Dialog then portals to `<body>` so a full-page screenshot
   captures it. Some overlays combine open + a store seed (skills dialogs call
   `openDrawer()` then `seedSkills(...)`).

3. **Bound props** (`lazyBound` in overlays / `lazyProps` in seeded). For
   prop-driven overlays whose visibility is a parent-passed `open` prop (not a
   store) — e.g. `{ open: true, onClose: noop }` for `ImportSkillDialog`,
   `AddToProjectModal`, all Import*/Run/DryRun/Tests dialogs, hub *Details drawers,
   `ProviderApiKeyModal`. Seeded uses `lazyProps` to feed fixed content into
   leaf components (`KitMarkdownEditor` initialMarkdown, `CsvGridEditor`
   initialText, `FileCard` uploadProgress, `McpToolCallsTab` serverId, …) and
   `lazyCompose` to stack several sections (web-search loading = 2 sections).

Deep-states use channel 1 exclusively via `useChatStore.setState` /
`useMcpComposerStore.getState().addToolCall(...)` / `useFileStore.setState` /
`ModelPicker.store.setState`, always after `whenLoaded(conversationId)` has
confirmed the real ConversationPage finished loading the fixture conversation.

Escape hatches used a few times: shard3 `seeded-s3-group-widget-error` installs a
**narrow one-time `window.fetch` shim** (500s only `GET /api/groups/:id/providers`)
because `LLMProviderGroupWidget` owns a `defineLocalStore` with no global setState
handle. shard1's panel/elicit surfaces **patch a store ACTION** (`WorkflowStoreDef.store.setState({ test: impl })`)
so the panel's local-`useState` outcome branch (loading/error/result) is driven by
the swapped action's resolve/reject/hang.

Registration wiring: `pages.tsx:235-242` `GalleryPages({only,state})` resolves the
`only` slug against `deepStateBySlug` → `seededSurfaceBySlug` → `overlayBySlug` (in
that order; a seeded slug that duplicates an enumerated page slug therefore SHADOWS
it). The three frames — `DeepStateFrame`/`SeededSurfaceFrame` (own files) and
`OverlayFrame` (`pages.tsx:197-228`) — mount the component, run `setup`/`open`, and
call `useRunInteraction`.

---

## 3. Registration boilerplate a NEW entry requires (modularization judgment)

- **Overlay:** append an `OverlayEntry` object to the `OVERLAY_ENTRIES` array
  literal in `overlays.tsx`. Requires: `slug`, `surface` (component-file id, feeds
  the coverage gate), `title`, `component` (via `lazyNamed`/`lazyBound`), and either
  `open` (store action) or bound props. Fixtures live in `overlays.tsx` inline or
  `./fixtures/*`. All entries are in ONE array in ONE file — no per-module split.
- **Deep-state:** append a `DeepStateEntry` to `DEEP_STATE_ENTRIES` in
  `deepStates.tsx`. Requires `slug`/`title`/`conversationId`/`note` + optional
  `setup`. `conversationId` must exist in `./fixtures/chat-deep`. One array, one file.
- **Seeded:** the ONLY class with a modular contract. Integrator entries go in the
  `integratorSeeded` array in `seededSurfaces.tsx`; sharded entries go in
  `seeded/shard<N>.tsx` exporting `shard<N>Seeded`, which `seededSurfaces.tsx`
  spreads into `SEEDED_SURFACE_ENTRIES` (`:1298-1305`). The shard header comment
  (`helpers.tsx:11-16`, echoed in each shard) is the "parallel-grind contract":
  each shard owns ONLY its file, slugs MUST be prefixed `seeded-s<N>-`, and shards
  MUST NOT edit `seededSurfaces.tsx`, `overlays.tsx`, `main.tsx`, `pages.tsx`,
  `stories/index.ts`, `coverage-allowlist.json`, or any generated matrix. To add a
  new shard: create `seeded/shard6.tsx` + add one import + one spread in
  `seededSurfaces.tsx`. This shard/aggregator split is the existing precedent for
  "modularize the seed authoring per module".

Every class ALSO exports `<class>BySlug(slug)` + a slug list constant (e.g.
`SEEDED_SURFACE_SLUGS`), which `pages.tsx` and the Node capture/coverage tools
enumerate. A new entry is auto-picked-up by these because they map over the arrays.

## 4. Lazy vs eager

**All entries reference their component lazily.** Every class stores
`component: LazyExoticComponent<ComponentType>`, built by `React.lazy(() => import(...))`
through the helpers `lazyNamed` / `lazyBound` (overlays) and `lazyNamed` /
`lazyProps` / `lazyCompose` (seeded, in `seeded/helpers.tsx`). Deep-states lazy the
single shared `ConversationPage` (`deepStates.tsx:39`). Fixture DATA objects
(`workflowFixture`, `hubModelFixture`, `deepProject`, canned test/dry-run results,
the base64 XLSX, etc.) are eager module-level consts, but the module CODE is always
`import()`-split. Shard `setup`/action bodies also `await import(...)` their stores
lazily. Frames wrap the component in `<Suspense fallback={<Loading/>}>` +
`AppErrorBoundary`.

---

## 5. Demo components (`DefectRepro`, `MessageListLongDemo`, `TableDemos`)

All three are gallery-LOCAL demo components (not real module pages), referenced
ONLY by the seeded class (via `lazyNamed(() => import('./…'), name)`), store-free:

- **`DefectRepro.tsx`** → seeded slug `seeded-defect-repro` (`fullHeight: true`).
  The detection system's living known-positive fixture suite: renders every
  geometry/runtime taxonomy miss as an intentionally-defective `repro-<class>-<slug>`
  cell for `scripts/detector-acceptance.mjs`. Allow-listed for the geometry gate.
- **`MessageListLongDemo.tsx`** (export `MessageListLongDemo`, `count=500`) →
  seeded slug `seeded-message-list-long`. Drives the REAL chat `MessageList`
  virtualizer with 500 mixed messages for the scroll-stability e2e.
- **`TableDemos.tsx`** (7 exports: `TableActionsDemo`, `TableScrollDemo`,
  `DelimitedViewerDemo`, `DelimitedViewerWithHeaderDemo`, `XlsxViewerDemo`,
  `LargeDelimitedViewerDemo`, `LargeRawCodeViewDemo`) → 8 seeded slugs
  (`seeded-kit-table-actions`, `-scroll`, `seeded-delimited-viewer`, `-shell`,
  `-large`, `seeded-xlsx-viewer`, `seeded-rawcode-large`). Isolated single-surface
  renders of real kit `Table` + `DelimitedTable`/`XlsxSheet`/`RawCodeView` viewers
  so the interactive F1 e2e can click sort/filter/resize/columns. Also shared by
  `stories/data.story.tsx` (browse canvas). None are referenced by overlays or deep.

---

## 6. FULL slug → class → module → how-seeded table

Module = the `src/modules/<X>` the surface belongs to (or `components/kit`,
`components/ui`, or `gallery-local` for gallery demo components).

### Overlays (44) — class `overlay`

| slug | module | how-seeded |
|---|---|---|
| overlay-llm-provider-drawer | llm-provider | store open: `Stores.LlmProviderDrawer.openLlmProviderDrawer(provider)` |
| overlay-create-user-drawer | user | store open: `CreateUserDrawer.openCreateUserDrawer()`; +interactions (focus-input, submit-invalid) |
| overlay-edit-user-drawer | user | store open: `EditUserDrawer.openEditUserDrawer(adminUser)` |
| overlay-reset-password-drawer | user | store open: `ResetPasswordDrawer.openResetPasswordDrawer(adminUser)` |
| overlay-edit-user-group-drawer | user | store open: `EditUserGroupDrawer.openUserGroupDrawer(group)` |
| overlay-assign-group-drawer | user | store open: `AssignGroupDrawer.openAssignGroupDrawer(adminUser)`; +interaction (submit-empty) |
| overlay-user-groups-drawer | user | store open: `UserGroupsDrawer.openUserGroupsDrawer(adminUser)` |
| overlay-group-members-drawer | user | store open: `GroupMembersDrawer.openGroupMembersDrawer(group)` |
| overlay-llm-repository-drawer | llm-repository | store open: `LlmRepositoryDrawer.openDrawer()` |
| overlay-group-llm-providers-assignment | llm-provider | store open: `GroupLlmProvidersAssignment.openDrawer(group)` |
| overlay-group-mcp-servers-assignment | mcp | store open: `GroupSystemMcpServersAssignment.openDrawer(group)` |
| overlay-group-skills-assignment | skill | store open: `GroupSystemSkillsAssignment.openDrawer(group)` |
| overlay-group-workflows-assignment | workflow | store open: `GroupSystemWorkflowsAssignment.openDrawer(group)` |
| overlay-assistant-form-drawer | assistant | store open: `AssistantDrawer.openAssistantDrawer()` |
| overlay-skills-conversation-loaded | skill | lazyBound props {conversationId} + `SkillConversationDrawer.openDrawer()` + `seedSkills(list,available)` (holdPatch); +interaction (open-detail) |
| overlay-skills-conversation-empty | skill | lazyBound props + openDrawer + `seedSkills([],[])` |
| overlay-skills-conversation-loading | skill | lazyBound props + openDrawer + `seedSkillsLoading()` |
| overlay-skill-detail-drawer | skill | `seedSkills(...)` + store open `SkillDrawer.open(skill, convId)` |
| overlay-import-skill-dialog | skill | lazyBound props {open:true,onClose} (no store) |
| overlay-file-preview-drawer | file | store open: `FilePreviewDrawer.openPreview(fileFixture)` |
| overlay-mcp-server-drawer | mcp | store open: `McpServerDrawer.openMcpServerDrawer()` |
| overlay-mcp-config-modal | mcp | store open: `McpComposer.openConfigModal()` |
| overlay-project-form-drawer | projects | store open: `ProjectDrawer.openProjectDrawer()` |
| overlay-add-to-project-modal | projects | lazyBound props {open,conversationId,onClose} (no store) |
| overlay-edit-llm-model-drawer | llm-provider | store open: `EditLlmModelDrawer.openEditLlmModelDrawer(id)` |
| overlay-add-remote-llm-model-drawer | llm-provider | store open: `AddRemoteLlmModelDrawer.openAddRemoteLlmModelDrawer(id,type)` |
| overlay-add-local-llm-model-upload-drawer | llm-provider | store open: `AddLocalLlmModelUploadDrawer.open…(providerId)` |
| overlay-add-local-llm-model-download-drawer | llm-provider | store open: `AddLocalLlmModelDownloadDrawer.open…(providerId)` |
| overlay-runtime-download-drawer | llm-local-runtime | store open: `RuntimeDownloadDrawer.openDrawer(engine)` |
| overlay-auth-provider-edit-drawer | auth-providers | lazyBound props {open,onClose} (no store) |
| overlay-provider-api-key-modal | user-llm-providers | lazyBound props {providerId,providerName,modelId,onSuccess,onCancel} (no store) |
| overlay-import-citations-modal | citations | lazyBound props {open,onClose,projectId} (no store) |
| overlay-workflow-detail-drawer | workflow | store open: `WorkflowDrawer.open(workflowFixture)` |
| overlay-import-workflow-dialog | workflow | lazyBound props {open,onClose} (no store) |
| overlay-workflow-run-dialog | workflow | lazyBound props {open,onClose,conversationId,workflow,onStarted} (no store) |
| overlay-dry-run-preview-dialog | workflow | lazyBound props {open,onClose,workflow} (no store) |
| overlay-workflow-tests-panel | workflow | lazyBound props {open,onClose,workflow} (no store) |
| overlay-hub-assistant-details-drawer | hub | lazyBound props {open,onClose,assistant} (no store) |
| overlay-hub-model-details-drawer | hub | store open: `ModelDetailsDrawer.open(hubModelFixture)` |
| overlay-hub-mcp-details-drawer | hub | store open: `McpServerDetailsDrawer.open(hubMcpFixture)` |
| overlay-hub-skill-details-drawer | hub | lazyBound props {open,onClose,item} (no store) |
| overlay-hub-workflow-details-drawer | hub | lazyBound props {open,onClose,item} (no store) |
| overlay-dialog-host-described | components/ui (kit/dialog-host) | imperative: `dialog.info({title,description,…})` |
| overlay-dialog-host-bare | components/ui (kit/dialog-host) | imperative: `dialog.warning({title,…})` (no description) |

### Deep-states (17) — class `deep` — ALL render chat `ConversationPage`

Module column = the store(s) the `setup` seeds; the rendered page is always
`modules/chat` (`ConversationPage` pinned to `conversationId`).

| slug | seeds store(s) | how-seeded |
|---|---|---|
| deep-chat-streaming | chat | whenLoaded + `useChatStore.setState` (streamingMessage/isStreaming); +interaction (open-plus-menu) |
| deep-chat-no-models | user-llm-providers (ModelPicker) | whenLoaded + `holdForever(ModelPicker.setState providers:[])`; +interaction (open-model-select) |
| deep-chat-tool-running | chat | `whenLoaded(toolRunning)` only (fixture conversation carries the state) |
| deep-chat-tool-failed | chat | `whenLoaded(toolFailed)` only |
| deep-chat-tool-group | chat | `whenLoaded(toolGroup)` only |
| deep-chat-mcp-toolcall-completed | mcp (McpComposer) | whenLoaded + `useMcpComposerStore.addToolCall({status:'completed'})` |
| deep-chat-mcp-toolcall-error | mcp (McpComposer) | whenLoaded + `addToolCall({status:'error'})`; +interaction (expand-details) |
| deep-chat-tool-approval | mcp (McpComposer) | whenLoaded + `addToolCall({status:'pending_approval'})` |
| deep-chat-attachments | chat | `whenLoaded(attachments)` only |
| deep-chat-elicitation | mcp (McpComposer) | whenLoaded + `addElicitationRequest(liveElicitation)` |
| deep-chat-ask-user-wizard | mcp (McpComposer) | whenLoaded + `addElicitationRequest(liveAskUser)` |
| deep-chat-right-panel-file | chat + file | whenLoaded + `useFileStore.setState` + `chat().displayInRightPanel({type:'file'})` |
| deep-chat-right-panel-literature | chat | whenLoaded + `chat().displayInRightPanel({type:'literature',data:literaturePanelData})` |
| deep-chat-right-panel-multi | chat + file | whenLoaded + file seed + TWO `displayInRightPanel` (file + literature) → tab strip |
| deep-chat-rendering-showcase | chat | `whenLoaded(RENDERING_SHOWCASE_ID)`; +interaction (html-preview) |
| deep-chat-branched | chat | whenLoaded + `useChatStore.setState(forkPoints)` |
| deep-chat-long | chat | `whenLoaded(SHOWCASE)` only; +interactions (rename, message-actions) |

### Seeded — integrator-owned (52) — class `seeded`

| slug | module | how-seeded |
|---|---|---|
| seeded-artifact-canvas-markdown | components/kit (editor) | lazyProps {initialMarkdown} — no store |
| seeded-artifact-canvas-image | components/kit (editor) | lazyProps {initialMarkdown w/ data-URL img} — no store |
| seeded-artifact-canvas-csv | file (CsvGridEditor) | lazyProps {initialText} — no store |
| seeded-artifact-canvas-code | components/kit (editor) | lazyProps {initialText} — no store |
| seeded-artifact-canvas-edit-body | file (FileEditBody) | lazyProps {file,onDone} — no store |
| seeded-interact-provider-header | llm-provider | `holdForever(LlmProvider.setState providers)`; +interaction (rename) |
| deep-project-detail | projects | `seedProjectDetail(rich)` (holdPatch ProjectDetail + ProjectFiles); shadows /projects/:id |
| deep-project-detail-empty | projects | `seedProjectDetail(empty)` |
| deep-project-detail-error | projects | `seedProjectDetail(project:null,error)` |
| seeded-file-rag-error | file-rag | whenTrue(settings loaded) + `holdPatch(FileRagAdmin.setState error)` |
| seeded-sandbox-limits-error | code-sandbox | whenTrue(limits) + `holdPatch(SandboxResourceLimits.setState error)` |
| seeded-sandbox-rootfs-disabled | code-sandbox | `holdPatch(SandboxRootfsVersions.setState availability:'disabled_in_config'+catalog)` |
| seeded-sandbox-limits-loading | code-sandbox | `holdPatch(SandboxResourceLimits.setState loading:true,limits:null)` |
| seeded-web-search-loading | web-search | lazyCompose(2 sections) + `holdPatch(WebSearchAdmin.setState loading,settings:null,providers:[])` |
| seeded-literature-loading | literature | `holdPatch(LitSearchAdmin.setState loading,connectors:[])` |
| seeded-download-indicator-empty | llm-provider | `holdPatch(LlmModelDownload.setState downloads:[])` |
| seeded-recent-convos-loading | chat | `holdPatch(ChatHistory.setState loading,isInitialized:false)` |
| seeded-recent-convos-empty | chat | `holdPatch(ChatHistory.setState loaded,recentConversations:[])` |
| seeded-live-logs-empty | llm-local-runtime | lazyProps {modelId} — no store |
| seeded-workflow-runs-empty | workflow | lazyProps {workflowId,onSelectRun} + `holdPatch(WorkflowRuns.setState runs:{wf:[]})` |
| seeded-conversation-skills-loading | skill | lazyProps {conversationId} + `holdPatch(ConversationSkills.setState loading)` |
| seeded-conversation-skills-error | skill | lazyProps + `holdPatch(ConversationSkills.setState error)` |
| seeded-conversation-skills-empty | skill | lazyProps + `holdPatch(Skill+ConversationSkills empty)` |
| seeded-core-memory-loading | memory | lazyProps {assistantId} + `holdPatch(CoreMemoryBlocks.setState loading)` |
| seeded-core-memory-empty | memory | lazyProps + `holdPatch(CoreMemoryBlocks.setState empty)` |
| seeded-mcp-tool-calls-error | mcp | lazyProps {serverId} + `holdPatch(McpToolCalls.setState error)` |
| seeded-llm-models-loading | llm-provider | whenTrue(providers) + `holdPatch(LlmProvider.setState llmModelsLoading[pid])`; path /gallery/:providerId |
| seeded-provider-buttons-loading | auth | `holdPatch(AuthProviders.setState isLoading,hasLoaded:false)` |
| seeded-provider-buttons-error | auth | `holdPatch(AuthProviders.setState error,providers:[])` |
| seeded-provider-buttons-empty | auth | `holdPatch(AuthProviders.setState providers:[])` |
| seeded-login-error | auth | `holdPatch(Auth.setState error)` |
| seeded-register-error | auth | `holdPatch(Auth.setState error)` |
| seeded-chat-message-empty | chat | lazyProps {message: contents:[]} — no store |
| seeded-message-list-empty | chat | `holdPatch(useChatStore.setState messages:new Map())` |
| seeded-step-artifacts-empty | workflow | lazyProps {runId,stepId,artifacts:[]} — no store |
| seeded-hardware-no-gpu | hardware | `holdPatch(Hardware.setState currentUsage gpu_devices:[])` |
| seeded-chat-history-list | chat | `holdForever(AppLayout.nativeScroll + ChatHistory loading)`; path /chat-history |
| settings | settings | SHADOW of /settings; no setup (renders SettingsPage at /settings/general) |
| hardware-monitor | hardware | SHADOW; `holdPatch(Hardware.setState full snapshot)` (SSE data seeded) |
| seeded-hardware-monitor-error | hardware | `holdPatch(Hardware.setState hardwareError,hardwareInfo:null)`; path /hardware-monitor |
| auth-link-account | auth | SHADOW; no setup — initialPath carries `?link_token=…` |
| seeded-defect-repro | gallery-local (DefectRepro) | fullHeight, no store — intentional-defect fixture cells |
| seeded-kit-table-actions | gallery-local (TableDemos) | no store — real kit Table |
| seeded-kit-table-scroll | gallery-local (TableDemos) | no store |
| seeded-delimited-viewer | gallery-local (TableDemos) | no store — real DelimitedTable |
| seeded-delimited-viewer-shell | gallery-local (TableDemos) | no store — DelimitedHeader + DelimitedTable |
| seeded-xlsx-viewer | gallery-local (TableDemos) | no store — real XlsxSheet |
| seeded-delimited-viewer-large | gallery-local (TableDemos) | no store — >10k-row DelimitedTable |
| seeded-rawcode-large | gallery-local (TableDemos) | no store — RawCodeView large file |
| seeded-message-list-long | gallery-local (MessageListLongDemo → chat MessageList) | no store — 500 mixed messages |
| seeded-mcp-tool-calls-loaded | mcp | lazyProps {serverId} + `holdPatch(McpToolCalls.setState calls[])` |
| seeded-memory-audit-loaded | memory | `holdPatch(MemoryAudit.setState entries[])` |

### Seeded — shard1 (10) — module `workflow`

| slug | module | how-seeded |
|---|---|---|
| seeded-s1-run-progress-error | workflow | lazyProps {runId} + `holdPatch(WorkflowRun.setState failed run)` |
| seeded-s1-run-progress-empty-steps | workflow | lazyProps + `holdPatch(WorkflowRun.setState running, no steps)` |
| seeded-s1-tests-loading | workflow | panelSurface — patch `Workflow.test` action to hang |
| seeded-s1-tests-error | workflow | panelSurface — patch `test` action to reject |
| seeded-s1-tests-result | workflow | panelSurface — patch `test` action to resolve cannedTestResult |
| seeded-s1-dry-run-loading | workflow | panelSurface — patch `Workflow.dryRun` to hang |
| seeded-s1-dry-run-error | workflow | panelSurface — patch `dryRun` to reject |
| seeded-s1-dry-run-result | workflow | panelSurface — patch `dryRun` to resolve cannedDryRunResult |
| seeded-s1-elicit-error | workflow | custom lazy wrapper auto-clicks Submit with blank required field |
| seeded-s1-array-empty | workflow | custom lazy wrapper: RHF Form w/ empty field array |

### Seeded — shard2 (10) — module `file`

| slug | module | how-seeded |
|---|---|---|
| seeded-s2-xlsx-error | file (viewers/tabular/XlsxBody) | lazyProps {file} + `seedBinary(CORRUPT_XLSX)` (holdPatch File.fileBinaryContents) |
| seeded-s2-xlsx-loading | file (XlsxBody) | lazyProps + holdPatch File: park id in fileBinaryLoadingSet, delete from contents |
| seeded-s2-xlsx-empty | file (XlsxBody) | lazyProps + `seedBinary(ZERO_SHEET_XLSX)` |
| seeded-s2-pdf-empty | file (viewers/pdf/body) | lazyProps {file: preview_page_count:0} — no store |
| seeded-s2-chrome-viewmode-fallback | file (viewers/shared/chrome) | lazyProps {file} + holdPatch delete fileViewModes entry |
| seeded-s2-filecard-row-error | file (FileCard) | lazyProps {variant:'row',uploadProgress:error,onRetry} — no store |
| seeded-s2-filecard-square-error | file (FileCard) | lazyProps {variant:'square',uploadProgress:error,onRetry} — no store |
| seeded-s2-project-files-inline-loading | file (project-extension) | lazyProps + `seedProjectFiles({files:[],filesLoading:true})` |
| seeded-s2-project-files-inline-empty | file (project-extension) | lazyProps + `seedProjectFiles({files:[],filesLoading:false})` |
| seeded-s2-project-files-manage-empty | file (project-extension) | lazyProps + `seedProjectFiles(empty)` |

### Seeded — shard3 (8) — modules `llm-provider` / `llm-local-runtime`

| slug | module | how-seeded |
|---|---|---|
| seeded-s3-download-view-failed | llm-provider (llm-models) | holdPatch LlmModelDownload(failed) + ViewDownloadDrawer(open) |
| seeded-s3-download-view-downloading | llm-provider (llm-models) | holdPatch LlmModelDownload(downloading) + ViewDownloadDrawer(open) |
| seeded-s3-available-versions-empty | llm-local-runtime | lazyProps {engine} + holdPatch RuntimeConfig(gpu) + RuntimeUpdate(versions:[]) |
| seeded-s3-available-versions-failed-row | llm-local-runtime | lazyProps + holdPatch RuntimeUpdate(ready version) + RuntimeDownloadProgress(failed) |
| seeded-s3-local-provider-loading | llm-provider | `holdForever(LlmProvider.setState loading,providers:[])`; path /gallery/:providerId |
| seeded-s3-downloads-section-empty | llm-provider (downloads) | lazyProps {providerId} + holdPatch LlmModelDownload(downloads:[]) |
| seeded-s3-version-models-empty | llm-local-runtime | lazyProps {engine,versionId,models:[]} — no store |
| seeded-s3-group-widget-error | llm-provider (widget) | custom lazy: one-time `window.fetch` shim 500s GET /api/groups/:id/providers |

### Seeded — shard4 (8) — modules `mcp` / `citations` / `literature`

| slug | module | how-seeded |
|---|---|---|
| seeded-s4-project-mcp-loading | mcp (project-extension) | lazyNamed + holdPatch ProjectDetail(project) + ProjectMcpSettings(loading,settings:null) |
| seeded-s4-project-mcp-empty | mcp (project-extension) | lazyNamed + holdPatch ProjectMcpSettings(no rules) |
| seeded-s4-mcp-user-policy-no-transports | mcp (system) | holdPatch AppMode(multiUser) + McpUserPolicy(allowed_transports:[]) |
| seeded-s4-kv-secret-empty | mcp (common) | custom lazy: RHF Form w/ empty headers_entries |
| seeded-s4-group-mcp-widget-error | mcp (widget) | lazyProps {group} + holdPatch GroupSystemMcpServersWidget(groupServers error) |
| seeded-s4-project-bib-manage-empty | citations (project-extension) | lazyNamed + `seedProject` (cassette returns entries:[]) |
| seeded-s4-project-bib-inline-empty | citations (project-extension) | lazyNamed + `seedProject` (count 0) |
| seeded-s4-lit-tool-result-empty | literature | lazyProps {content: structured_content records:[]} — no store |

### Seeded — shard5 (6) — modules `chat` / `auth` / `user-profile` / `projects`

| slug | module | how-seeded |
|---|---|---|
| seeded-s5-conversation-loading | chat | lazyNamed(default) + holdPatch useChatStore(loading,conversation:null); path /chat/:id |
| seeded-s5-conversation-not-found | chat | holdPatch useChatStore(!loading,conversation:null) |
| seeded-s5-conversation-error | chat | loadConversation + whenTrue + holdPatch useChatStore(error) |
| seeded-s5-auth-initializing | auth (AuthGuard) | lazyProps {children:null} + holdPatch AppMode+App(needsSetup:null)+Auth(isInitializing) |
| seeded-s5-user-profile-loading | user-profile | holdForever(Auth.setState user:null,isInitializing:true) |
| seeded-s5-project-form-loading | projects | holdPatch ProjectDrawer(open,loading) + dispatch Escape keydowns (loading-guard) |

---

## 7. Modules represented across the three classes (for modularization)

Union of modules touched (rendered component OR seeded store):

- **user** — 6 overlays
- **llm-provider** — 5 overlays + 6 seeded (integrator+s3)
- **workflow** — 5 overlays + 1 seeded(integrator) + 10 seeded(s1) + workflow-assignment overlay
- **skill** — 5 overlays + 3 seeded(integrator)
- **mcp** — 3 overlays + 2 seeded(integrator) + 5 seeded(s4) + (deep McpComposer seeds)
- **hub** — 5 overlays
- **auth** — 1 overlay(auth-providers) + 7 seeded (5 integrator + s5 AuthGuard) + link-account/login/register
- **file** — 1 overlay + several seeded(integrator) + 10 seeded(s2) + (deep File seed)
- **projects** — 2 overlays + 4 seeded(integrator ProjectDetail x3) + s5 + (s2/s4 project seeds)
- **citations** — 1 overlay + 2 seeded(s4)
- **literature** — 1 seeded(integrator) + 1 seeded(s4)
- **llm-local-runtime** — 1 overlay + 1 seeded(integrator) + 3 seeded(s3)
- **llm-repository** — 1 overlay
- **assistant** — 1 overlay
- **user-llm-providers** — 1 overlay + (deep ModelPicker seed)
- **chat** — all 17 deep-states + ~7 seeded(integrator) + 3 seeded(s5)
- **memory** — 3 seeded(integrator)
- **hardware** — 3 seeded(integrator)
- **file-rag**, **code-sandbox**, **web-search**, **settings**, **user-profile** — seeded only
- **components/kit** (editor), **components/ui** (dialog-host / kit Table) — cross-cut
- **gallery-local** (DefectRepro, TableDemos ×7, MessageListLongDemo) — 10 seeded slugs

Key modularization observations:
- Overlays + deep-states are single-file, single-array (no per-module split); the
  ONLY existing modular precedent is the seeded **shard** contract
  (`seeded/shard<N>.tsx` + `seeded/helpers.tsx`, slug-prefixed, aggregator-spread).
- The seed CHANNEL is uniform across all three classes: real component + real
  store `setState` with `holdPatch`/`holdForever`/`whenTrue` durability. NO
  per-entry mock-API cassette override; the cassette answers GETs and the entry
  layers the transient/mutation-failure state on top.
- Deep is a hard-coded specialization of seeded (component=ConversationPage). If
  modularizing "per module", deep could fold into a chat shard; overlays could
  adopt the same shard/aggregator split seeded already uses.
