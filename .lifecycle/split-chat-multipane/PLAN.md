# PLAN — split-chat-multipane

**Goal.** Split-screen chat: 2+ conversations open side-by-side, each fully
independent — its own composer/input, own live streaming, own scroll position,
own right-panel. The blocker is that chat state is a **singleton**
(`Stores.Chat` holds ONE conversation; `ChatInput` / `MessageList` / streaming /
right-panel / the whole chat-extension system are keyed to that one active
conversation). This refactor makes the per-conversation chat runtime
**multi-instance** (per-pane) while keeping the genuinely-global state global.

## v2 REDESIGN — the workspace interaction model (supersedes v1's open/navigate/persist layer)

**Why.** v1 shipped the per-pane *engine* (ITEM-2..10, 16-23 — 8/8, verified,
**reused unchanged**) but its *interaction layer* was a design hole: the Split
button only ever produced `[current | new-chat]`, panes were derived ad-hoc from
the URL, and **there was no way to place two EXISTING conversations side by
side** — the feature's own headline. Human review (see HUMAN_FEEDBACK FB-2/FB-3)
sent this back to Phase 1. v2 rebuilds the open/navigate/persist layer as a
first-class **workspace**, on top of the reused engine.

**Locked model (user decisions, DEC-40..43 below):** a **Workspace** = a
persistent set of 1..N open conversations (one per pane), IDE-editor-groups
style; **plain sidebar-click = replace the focused pane**, modifier/middle-click
= new pane; **localStorage per-user** persistence; affordances = **all four**
(empty-pane picker + ⋯-menu item + drag-drop + modifier-click). The URL is a
*view into* the workspace (the focused pane's conversation), never its source of
truth.

**The v2 items (build ON the reused engine):**

- **ITEM-24**: **Workspace store** — evolve the `SplitView` store into the
  persistent workspace source of truth: stable `paneId`s,
  `panes:[{paneId,conversationId|null}]`, `focusedPaneId`, `dividerWidths`,
  `mode:'columns'|'tabs'`; **enforce one conversation per pane** (adding a conv
  already open focuses its pane, never duplicates). *Revises ITEM-1* — DROP the
  `?pane=<id>` URL mirroring; the layout lives in the store + localStorage only.
- **ITEM-25**: **The reconciliation reducer** — one pure
  `openConversationInWorkspace(convId, intent ∈ 'auto'|'newPane'|'replaceFocused')`
  that EVERY entry point routes through: already-in-a-pane → **focus that pane**;
  `newPane` (modifier) → **add a pane** (MAX_PANES-guarded); `replaceFocused`/
  `auto` while a split is open → **replace the focused pane**; no split open →
  normal single-pane navigate. Pure + unit-testable. *Supersedes v1's flawed open
  path* (`openPane(current)+openPane(null)`), which is the root cause.
- **ITEM-26**: **Persistence + hydrate/prune** — persist the workspace to
  `ziee-split-workspace-v2`, namespaced by `Stores.Auth.user.id` (mirroring
  `chatDrafts` keying); hydrate on boot; on hydrate AND on `sync:conversation`
  delete, **prune** panes whose conversation is deleted / not-accessible, and
  prune empty panes (fall back to single-pane if it empties); include a v1→v2 key
  migration. *Revises ITEM-1's* localStorage shape. Answers "store the pane state"
  + "leave to another page then return" (rehydrate + ITEM-25 reconcile the URL).
- **ITEM-27**: **Empty-pane picker** — a `conversationId:null` pane renders a
  searchable "Open a conversation" list + a "Start a new chat" action; picking →
  `setPaneConversation`, new → the existing new-chat adopt path. Replaces the bare
  new-chat second pane so the second slot can hold an EXISTING conversation. New
  component `ConversationPickerPane.tsx`.
- **ITEM-28**: **Sidebar/list reroute + affordances** — `RecentConversationsWidget`
  + `ConversationCard` clicks call ITEM-25 (`intent:'auto'`) instead of raw
  `navigate`; **Cmd/Ctrl/middle-click → `intent:'newPane'`**; add an **"Open in
  split pane"** ⋯-menu item (→ `newPane`); the **Split button** opens
  `[current | empty-picker pane]`. *Supersedes ITEM-11's* "defaulting to a new
  chat". Answers the two sidebar-click questions (in-pane → focus; not-in-pane →
  replace focused / modifier→new).
- **ITEM-29**: **Edge-case semantics** — **pop-out MOVES** the pane out of the
  workspace (no two live copies competing); **delete/no-access** on a paned conv
  auto-closes that pane (+ toast, reusing per-pane sync + self-gate); **close-to-1**
  exits split to the survivor's single-pane view (workspace persists, re-expandable);
  focus reassigns to a neighbor on close; **MAX_PANES** over-cap → toast + offer
  replace-focused. *Revises ITEM-11/13/14* (pop-out/close/limits) for the
  workspace model.
- **ITEM-30**: **Mobile tab strip (build the deferred piece)** — below the
  `useWindowMinSize` breakpoint, `mode:'tabs'` renders ONE visible pane + a tab per
  open conversation (no columns, divider hidden); "open beside" becomes "add a
  tab"; focus = the visible tab. *Implements the v1-deferred ITEM-12/DRIFT-1.11.*
- **ITEM-31**: **Drag-and-drop (full model)** — drag a sidebar conversation onto a
  pane (drop = replace that pane) or the inter-pane seam / edge (drop = new pane at
  that index); drag a pane header to `reorderPanes`. *Implements the v1-deferred
  ITEM-16/DRIFT-1.10*; pointer-based, tokens for ghost/drop highlights.

- **ITEM-32**: **Complete the per-pane composer migration (the FB-4 fix).** v1
  migrated ONLY `TextInput` (the textarea draft) to `useChatPane()`; **~16 other
  composer/pane components still read `Stores.Chat` directly** — `ChatInput`
  (send + `useSendBlockers`), `ModelSelector`, `AssistantSelector` + menu/chip,
  `FilePreviewList`/`FileUploadButton`/`InlineFilePreview`/`AttachedFileCard`, the
  `MemoryStatusPill`/`SummarizationStatusPill`, `ToolCallPendingApprovalContent`,
  `TextContent`, `WorkflowWorkspaceRunCard`, `ProjectConversationsList`. The
  context-aware bridge makes reactive READS per-pane but resolves **actions +
  `.$`/getState/setState/subscribe to `focusedApi()`** — so their Send, model/
  assistant selection, file attach, and send-blockers effectively target the
  FOCUSED pane, not their own (two new-chat panes even share the `NEW_CHAT`
  model/assistant key). **Bind every pane-scoped composer component to
  `useChatPane().store`** so its actions + snapshots hit ITS pane, and make the
  FIVE composer stores genuinely per-pane (TextStore done; **File / ModelPicker /
  AssistantPicker / McpComposer must be per-pane-instanced or pane-keyed, NOT
  follow-focus** — this **supersedes DRIFT-1.2/1.3/1.4**, which left them
  focus-following, and **corrects DRIFT-1.7's** claim that the bridge obviated
  migrating ~40 consumers: true for reactive reads, FALSE for actions/snapshots).
  The bridge is retained ONLY for genuinely out-of-subtree consumers (DEC-5).
  *This is the v1 ITEM-5/ITEM-10 work made real + verified.*

*Reused unchanged (the audit treats these as the foundation):* the per-pane
`ChatPaneStore`/`ChatPaneProvider`, per-pane streaming/scroll/right-panel
isolation (ITEM-2..7 view + ITEM-6 stream + ITEM-18 right-panel), the pop-out
util (ITEM-P1..P4), and the divider. **NOT reused unchanged — corrected by
ITEM-32:** the composer/extension per-pane binding (ITEM-5/10/19) was only
partially done in v1 (bridge-reliant); ITEM-32 completes it. The `Stores.Chat`
bridge (ITEM-9) stays but narrows to out-of-subtree use only.

## Delivery phases

**Both pop-out AND in-window split are committed deliverables of this feature.**
They ship in **two tranches on this branch**, sequenced only for risk-ordering
(the low-risk pop-out lands first and can be its own PR); the split is not
optional and follows in the same feature:

- **Phase 1 — pop-out window/tab (`ITEM-P1..P4`).** "Open in new window"
  (desktop: a native Tauri `WebviewWindow`; web: `window.open`). Each new
  top-level window/tab boots a FRESH copy of the SPA with its OWN singleton
  `Stores.Chat`, its own SSE stream and extensions — so it is fully independent
  **with essentially no chat refactor** (the singleton problem does not exist
  *per window*). Low-risk, all-platform, desktop-native; lands first.
- **Phase 2 — in-window split (`ITEM-1..15`).** 2+ conversations side-by-side
  inside ONE app window with a draggable divider — the heavy per-pane refactor
  below. Its unique value over pop-out is side-by-side tiling in one frame / one
  taskbar entry. Lands after Phase 1; the two affordances coexist.

## Findings that shape the plan (from the codebase study)

- **The per-conversation store is `src-app/ui/src/modules/chat/core/stores/Chat.store.ts`**,
  a `defineStore('Chat')` singleton. It already *juggles* multiple conversations
  via `conversationStateCache` (whole-store snapshots swapped in/out of the one
  active slot) — the split makes N of those snapshots **live simultaneously**.
- **Streaming is already conversation-id-tagged.** `ChatStreamClient` emits
  `chat:token` carrying `{conversation_id, event}`; `applyStreamFrame` guards
  every mutation on `get().conversation?.id === conversationId`. The per-pane
  routing target is that single `conversation` field.
- **The backend `chat/stream` registry already supports the multi-pane shape
  with ZERO change**: `by_user` holds up to 12 connections/user, each scoped to
  ONE conversation (`ChatConn.active_conversation: Option<Uuid>`); `publish_frame`
  / `publish_raw_event` deliver only to connections subscribed to the frame's
  conversation; generations are keyed per-conversation. → We adopt **one SSE
  connection per pane** (a per-pane `ChatStreamClient` instance), which reuses
  the existing per-connection scoping *exactly* and correctly attributes the
  **unenveloped** raw extension events (each connection = one conversation).
  **This feature is ALMOST frontend-only** (no migration, no OpenAPI regen) — the
  streaming re-audit found TWO backend caveats (ITEM-20): (a)
  `PER_USER_MAX_CONNECTIONS = 12` is a hardcoded const — one connection per pane
  caps a user at ~12 panes across devices and reconnect churn can 429 a legit
  pane, so it must be raised + made configurable; (b) raw SSE events carry no
  `conversation_id`, so a pane that SWITCHES its conversation must open a FRESH
  connection (never repoint) or a stale buffered raw event mis-attributes. All
  other backend paths (begin_generation, stop-by-message-id,
  update_conversation_state, approval-by-URL, elicitation-by-random-id) are
  verified per-conversation/per-message — no per-user active-conversation
  singleton exists.
- **`defineLocalStore` is the sanctioned multi-instance primitive** (store-kit):
  `.use({conversationId})` per mount, isolated EventBus group `local:<n>`, init
  on mount / destroy on unmount. Sole production precedent:
  `LlmProviderGroupWidget.store.ts` (one instance per group row) — the template.
- **The chat-extension system is the deepest coupling.** `chatExtensionRegistry`
  is a module singleton holding two *kinds* of state: (a) **global registration
  descriptors** (extensions map, slots, content-types, SSE handlers, delta
  processors, content providers — populated once at module load), and (b)
  **per-conversation runtime** (the `initialized` flag + each extension's ONE
  store instance, injected into `Stores.Chat.<Name>`). Extensions reach the store
  by importing the global `useChatStore` and calling `.subscribe()`/`.getState()`
  in a zero-arg `initialize()` (file / mcp / user-llm-providers / assistant do
  this). To make panes independent, (a) stays global; (b) must become per-pane.
- **CORRECTION (tree-fix): a full message-virtualization + window-pagination
  subsystem DOES exist on origin/main.** The original study read a stale checkout
  (`/data/pbya/ziee/ziee` @ `786b26890`, 28 commits behind); the worktree is now
  reset to origin/main (`90e715c12`). Real subsystem: `MessageList.tsx` uses
  `@tanstack/react-virtual` (`useVirtualizer` + imperative `MessageListHandle`:
  `scrollToMessageId`/`scrollToBottom`/`captureAnchor`/`restoreAnchor`); the Chat
  store has `hasMoreBefore/After`, `loadingOlder/Newer`,
  `loadOlderMessages`/`loadNewerMessages`/`jumpToMessage`/`reconcileTail` +
  `messageWindow.ts` (prepend/append) with `MESSAGE_PAGE_SIZE=30`;
  `ConversationPage.tsx` runs top/bottom-sentinel `IntersectionObserver`s (800px
  prefetch) + at-bottom follow + `#message-<id>` deep-link + Cmd-F. There is a
  SECOND per-conversation store `MessageViewState` (collapse/inline-file view
  state). This reshapes ITEM-2 (store now owns window pagination) + ITEM-7
  (per-pane virtualizer + observers + imperative handle) and adds ITEM-21
  (`MessageViewState`). GLOBAL-SAFE (keep one copy, id×bucket / WeakMap keyed):
  `measuredHeightCache`, `estimateMessageHeight`, and the pure helpers
  (`messageWindow`/`scrollAnchor.utils`/`branchAnchor.utils`). Also corrected:
  `.__state` is ALREADY migrated to `.$` here (0 `Stores.Chat.__state`, not 24 —
  ITEM-10's premise was stale). `pendingProjectId` still does not exist.

## Items

**Phase 1 — pop-out window/tab (ships first, ~no chat refactor):**

- **ITEM-P1**: Platform pop-out util `openConversationWindow(conversationId, { title })` (new `modules/chat/core/popout/openConversationWindow.ts`) — on desktop (`window.__TAURI__`) construct a Tauri `WebviewWindow` labelled `chat-<conversationId>` at url `/chat/<conversationId>`; on web call `window.open('/chat/<conversationId>', '_blank')`. Opening a native OS window is shell-native, so it uses the Tauri window API directly (NOT an Axum route). Returns/focuses an existing window for the same conversation instead of duplicating.
- **ITEM-P2**: "Open in new window" affordance — a conversation-header action + a conversation-list / `RecentConversationsWidget` row-menu item, both calling `openConversationWindow`. Copy adapts per platform ("Open in new window" on desktop, "Open in new tab" on web). Sits beside the Phase-2 "Split" affordance (they coexist).
- **ITEM-P3**: New-window bootstrap correctness — verify a freshly-spawned window authenticates itself (desktop: the existing `desktop-base` → `invoke('auto_login')`; web: the shared httpOnly session cookie + silent refresh) and deep-links to `/chat/:conversationId` rendering the existing single-conversation `ConversationPage`; confirm the new window opens its own `chat/stream` SSE (within the server's 12-conn/user cap) and that a message sent/edited in one window reconciles into the other via the existing notify-and-refetch sync stream.
- **ITEM-P4**: Pop-out window lifecycle + chrome — dedup (reopening a conversation already in a window focuses it via the `chat-<id>` label); the desktop window's title tracks the conversation title (reusing the existing `conversation.titleUpdated` path) with the app's standard decorations/drag-region/min-size; window close tears down its SPA (SSE/stores) via the normal unload path.

**Phase 2 — in-window split (per-pane refactor):**

- **ITEM-1**: `SplitView` global layout store (`defineStore('SplitView')`, immer) — the only NEW global state: an ordered `panes: [{paneId, conversationId|null, projectId|null}]`, `focusedPaneId`, `direction`, per-divider widths, and `mode` (split|tabs). Actions: `openPane` / `closePane` / `focusPane` / `setPaneConversation` / `reorderPanes` / `setDividerWidth` / `setMode`. Persists layout to localStorage and mirrors it to the URL query (`?pane=<id>`), mirroring the right-panel persistence pattern.
- **ITEM-2**: Convert `Chat.store.ts` from the `defineStore('Chat')` singleton to a `ChatPaneStore` per-pane `defineLocalStore` def, instantiated per pane via `.use({ conversationId, paneId })`. Preserve ALL per-conversation state and actions verbatim (conversation, messages, streaming assembly, branches, forkPoints, editingMessage, rightPanel, loadConversation switch logic + `conversationStateCache`). Move the module-globals INTO the instance: `frameApplyTail` (per-pane serialization) AND `lastChatResyncAt`/`CHAT_RESYNC_MIN_INTERVAL_MS` (per-pane resync debounce — audit GAP-7). Convert the raw `Stores.EventBus.on(..., 'Chat')` listeners (sync:conversation, sync:reconnect, chat:stream-reconnect) to the store-kit `ctx.on` seam so each instance gets its OWN `local:<n>` EventBus group — the shared `'Chat'` group + `removeGroupListeners('Chat')` would otherwise tear down EVERY pane's listeners when one unmounts (audit GAP-5). Preserve the `cacheClearTimers` teardown loop in the instance `onCleanup` (else panes leak timers). The `applyStreamFrame` `conversation?.id === conversationId` guards stay as a safety net (routing is now the direct per-pane callback, ITEM-6). **The store also owns the window-pagination subsystem (tree-fix):** `hasMoreBefore/After`, `loadingOlder/Newer`, `loadMessages`/`loadOlderMessages`/`loadNewerMessages`/`jumpToMessage`/`reconcileTail`, `MESSAGE_PAGE_SIZE`, and the `ChatStateSnapshot` that preserves `hasMore*` — all per-pane. **Round-2 feasibility constraints:** (a) de-async the store's `init` (hoist the dynamic imports; the async IIFE + `defineLocalStore`'s mount/unmount lifecycle race a fast mount→unmount into a listener leak); (b) `.use({conversationId})` does NOT re-init on a conversation change (ref-frozen instance) — the `ChatPaneProvider` must carry a `useEffect(() => store.loadConversation(cid), [cid])` (+ the ITEM-6 stream reopen) for the DEC-14 in-pane switch; (c) `onFrame` must chain through the per-instance `frameApplyTail` via an `enqueueStreamFrame` action — NOT call `applyStreamFrame` directly (fixes the ARCHITECTURE §3 contradiction that would reintroduce the token-interleave bug); (d) deep-clone the initial nested Maps per `.use()` (shared `config.state` object) or document copy-on-write; (e) the singleton's 5s destroy-grace is lost — validate StrictMode double-mount.
- **ITEM-3**: `ChatPaneProvider` React context + `useChatPane()` hook (new `modules/chat/core/pane/`) — instantiates and provides, for a pane subtree: the pane's `ChatPaneStore` instance, its `PaneExtensionRuntime` (ITEM-4), and its `ChatStreamClient` (ITEM-6). Owns the pane lifecycle (mount → store init + stream connect scoped to the pane's conversation; unmount → destroy + disconnect).
- **ITEM-4**: Split `ChatExtensionRegistry` into a global `ExtensionCatalog` (the registration descriptors — extensions map, slotRegistry, contentTypeRegistry, sseEventHandlerRegistry, streamingDeltaProcessorRegistry, streamingContentProviderRegistry; `register()` still called once per extension at module load) and a per-pane `PaneExtensionRuntime` (constructs each extension's store instance for THIS pane, holds the `initialized` flag, and runs the lifecycle/hook methods — `initialize` / `cleanup` / `onConversationLoad` / `beforeSendMessage` / `composeRequestFields` / `handleSSEEvent` / `provideStreamingContent` / `afterStreamComplete` / `afterCreateConversation` / `onStreamStart|Error` / `onMessageSent` / `onMessageEditRestore` — against THIS pane's chat store). **The extension-store `createStore()` moves from boot-time `register()` (which today makes ONE injected `Stores.Chat.<Name>`) to per-pane runtime mount** — `register()` records the descriptor + factory only; the runtime calls the factory per pane (audit GAP-15). Each runtime holds its OWN `initialized` flag — the singleton registry's shared `initialized` (which makes the 2nd pane's `initialize()` early-return "Already initialized", silently skipping its subscriptions + `registerPanelRenderer`) is eliminated (audit GAP-1). **The #1 hot-path change:** `handleSSEEvent` passes each extension `sseEventHandler` a `(data, get, set)` where `get`/`set` are hardwired to `useChatStore.getState/setState` (`registry.tsx:718`/`:719`, invoked at `:732`; same singleton binding recurs at `:105-110`/`:285-287`) — rebind these to the PANE's chat store so SSE writes land in the right pane. Same for `renderContent`/`renderSlot`/`useSendBlockers`/`useConversationMenuContributions`, whose returned components/hooks read `Stores.X` at render — they resolve the pane via context. The content-type co-ownership ordering (`tool_use` grouping; `tool_result` workflow→literature→file catch-all) is content-based and stays global.
- **ITEM-5**: Migrate the chat-extension API so extensions bind to a **pane** instead of the global singleton: give `initialize` (and the store-reaching hooks) a `ctx` carrying the pane's chat-store handle + the pane's extension-store instance, and rewrite every chat extension that currently imports the global `useChatStore` (text, file, mcp, user-llm-providers, assistant, memory, summarization, literature, project, skill, export, keyboard, title, syntax) to use `ctx.chatStore` / `ctx.store` instead. Extension stores continue to be created via `defineExtensionStore` (already per-call instances) — one set per pane runtime. **EVERY composer/conversation-scoped store must go per-pane — there are FIVE, not one (audit GAP-2).** The send path + composer read EXACTLY FIVE (confirmed by the consumer grep — no sixth): `TextStore` (draft text — the ONLY one still nested under `Stores.Chat`, via `extension.store`), and the TOP-LEVEL singletons `Stores.File` (attachments — the `Stores.Chat.FileStore` name is dead/comments-only), `Stores.ModelPicker` (model), `Stores.AssistantPicker` (assistant), `Stores.McpComposer` (MCP servers + tool-call/approval/elicitation state — see ITEM-19). `Stores.McpServer` is read in the send path but is a deployment-wide registry — it stays GLOBAL. Also global (id-keyed, do NOT pane-scope): `File.messageFilesCache`/`thumbnailUrls`, `McpComposer.conversationConfigs`, the project `conversationProjectCache`. Each becomes a per-pane instance owned by the pane runtime; `ModelSelector`/`composeRequestFields`/etc. resolve the pane's instance via `ctx`, so each pane holds its own draft/files/model/assistant/MCP selection. The extensions' `useChatStore.subscribe(conversation?.id / editingMessage)` wirings (file/mcp/assistant/user-llm) bind to the OWNING pane's store, not the singleton (audit GAP-10). **Clean cut (DEC-30):** all extensions migrate in one pass and the `useChatStore` singleton export is REMOVED — no focused-pane shim, no residual singleton coupling. (export + keyboard are extensions, so they too become pane-scoped via `ctx`; keyboard's DOM `querySelector` is scoped to the focused pane's root.)
- **ITEM-6**: Refactor `ChatStreamClient.ts` from a module singleton (module-level `started` / `connectionId` / `desiredConversationId`) into a `createChatStreamClient()` factory that owns ONE SSE connection scoped to ONE conversation. **Replace the global `chat:token` EventBus fan-out with a DIRECT per-pane callback** (`onFrame(cid,e) → thisPane.store.applyStreamFrame`), so a pane never receives another pane's frames — this dissolves the pre-guard extension-dispatch duplication (audit GAP-6) without reordering guards. Each pane client has its OWN `desiredConversationId`, so per-pane `setActiveConversation` / `reset(null)` only reprogram THAT pane's connection, never the device (audit GAP-3/4). **On a pane conversation SWITCH, tear the connection down and open a FRESH one (never repoint an existing connection)** — raw SSE events carry no `conversation_id`, so a stale buffered raw event would mis-attribute across a repoint (streaming-audit GAP-1); a fresh connection has an empty buffer. The `sseEventHandlers(data, get, set)` `get`/`set` (today hardwired to the singleton at `registry.tsx:717`) are rebound to the pane's chat store by the runtime (ITEM-4). The auth guard (`streamAuthGuard`) that starts/stops all clients on login/logout/user-switch lives at **app/module boot** and fires exactly ONCE device-level (relocated `chatStreamWired` + `useAuthStore.subscribe`), NOT in any pane's `init` (audit GAP-14).
- **ITEM-7**: Extract the single-conversation view from `ConversationPage.tsx` into a `ChatPane` component (header/`TitleEditor`, `message_list_header` slot, the **virtualized `MessageList`**, `ChatInput`, `ChatRightPanel`, the scroll effects) resolving its store via `useChatPane()`. Most of it instances per-pane FOR FREE (each pane has its own `DivScrollY`/`scrollerRef` viewport → its own `useVirtualizer`, top/bottom/at-bottom `IntersectionObserver`s, `pendingAnchorRef`/`messageListRef`/`MessageListHandle`). The concrete collisions to FIX (tree-fix): (1) `inPlaceAnchorSignal` (module singleton in `useInPlaceAnchor.ts:22`, shared with the virtualizer's scroll-adjust predicate, doc assumes one MessageList) → per-pane/per-virtualizer value; (2) the Cmd/Ctrl-F `window` keydown (both panes would open find bars) → scope to the focused pane; (3) the `#message-<id>` `hashchange` deep-link on the single `window.location.hash` → a per-pane ownership rule; (4) the native-scroll composer-auto-hide `window` scroll listener + `AppLayout.nativeScroll` (one window/mode) → gate to one pane (split is desktop inner-scroll); (5) DEV `window.__MSGLIST_METRICS__` + the non-virtualized `scrollToMessageId` document-wide `querySelector` → per-pane/scoped. `ConversationPage` becomes a thin wrapper rendering one pane inside `SplitChatView`.
- **ITEM-8**: `SplitChatView` container + route wiring — renders the **tab-strip workspace** chrome (design-tournament winner, DEC-24): a browser-like pane-tab strip (tabs with live-dot + close ✕ + "＋", and a strip-right toolbar with a distinct "Split ▐▌" and "New window ↗" per DEC-27) over framed workspace tiles on a muted background, separated by vertical `ResizeHandle` dividers (reused), with an active-tile ring + click-to-focus (DEC-28). Wires `/chat/:conversationId` (and the projects `/projects/:projectId/chat/:conversationId`) to render `SplitChatView`; the primary pane comes from the URL param, additional panes from the `?pane=<id>` query + the `SplitView` store.
- **ITEM-9**: `Stores.Chat` focused-pane **bridge** (narrow) + `paneRegistry` — a module-level `paneRegistry: Map<paneId, ChatPaneHandle>` (populated by each `ChatPaneProvider` on mount, removed on unmount) resolves `SplitView.focusedPaneId` → the live pane handle. `Stores.Chat` becomes a thin proxy over that focused handle doing **snapshot + action** forwarding, plus a `useFocusedChatPane()` hook for the ~2 out-of-subtree **reactive** reads. Per DEC-5/DEC-30 the bridge serves ONLY non-extension, non-subtree consumers (desktop `ConversationMountsControl`, dev-gallery fixtures) — extensions bind via `ctx`, pane components via `useChatPane()`. Document the boundary.
- **ITEM-10**: Migrate the pane-scoped consumers from `Stores.Chat` to `useChatPane()` — the chat view components (`ChatInput`, `MessageList`, `ChatMessage`, `BranchNavigator`, `EditingMessageBanner`, `MessageActions`, `TextContent`, `TitleEditor`, `ChatRightPanel`) and the in-pane extension slot components (text `TextInput`, file `FilePreviewList`/`FileUploadButton`/`InlineFilePreview`, `ModelSelector`, the status pills, mcp `ToolCallPendingApprovalContent`, `LiteratureScreeningPanel`, project chip) so each reads/acts on ITS pane. This includes converting the snapshot reads (on origin/main these are already `Stores.Chat.$` — the `.__state` sweep landed; ~13 files use `.$`) to pane reads, the desktop `ConversationMountsControl` (renders in the per-pane header via the `chatConversationHeaderTrailing` slot → `useChatPane()`, NOT the bridge — corrects DEC-5), the `toolbar_status` pills (`MemoryStatusPill`/`SummarizationStatusPill` read `Stores.Chat.conversation`), the `message_list_header` project chip, and both `TextContent` renderers (`isStreaming`→`isAnimating` currently animates every pane when any streams). The sidebar's selected-row highlight must derive from `SplitView.panes` (all open panes), not `location.pathname` (one URL). Also `ConversationFindBar.tsx` (renders in the pane subtree; reads `Stores.Chat.conversation`/`$.messages`/`jumpToMessage`) → `useChatPane()`, else pane B's find/jump would target the focused pane.
- **ITEM-11**: Open-in-split affordances — a "Split" button in the conversation header (opens a 2nd pane, defaulting to a new chat), an "Open in split pane" item in the conversation-list / `RecentConversationsWidget` row menu, and per-pane header controls (focus target, close pane). Enforce `MAX_PANES` (ITEM-14).
- **ITEM-12**: Mobile / small-screen behavior — below the `useWindowMinSize` breakpoint, collapse the split to a single **focused** pane plus a tab strip to switch panes (no simultaneous columns; the right panel already goes full-screen overlay on mobile). Open-in-split affordances add a tab instead of a column. Driven by `SplitView.mode = 'tabs'`.
- **ITEM-13**: Per-pane project context — replace the project chat-extension's `window.location.pathname` project-id derivation (`projectIdFromUrl`) with a pane-scoped `projectId` carried on the pane (from the pane's route/creation context), so a pane hosting a project conversation binds to the correct project independently of the URL and of the other panes' projects.
- **ITEM-14**: Layout limits — introduce a named `SPLIT_LIMITS` object (`MAX_PANES`, `MIN_PANE_WIDTH`, `MAX_PANE_WIDTH`, `DEFAULT_DIRECTION`) rather than inline magic numbers; `MAX_PANES` is a fixed frontend constant bounded by the existing server-side 12-connection-per-user cap (see DECISIONS DEC-15).
- **ITEM-16**: Unified drag interaction layer (`modules/chat/core/pane/paneDnd.ts` + drop-zone components) — dragging a conversation (from the sidebar) or a pane-tab surfaces (a) in-tile **edge drop-zones** (Split-left / Replace / Split-right) that open/move a pane, and (b) tab-strip **reorder** with a drop indicator; on drop it calls the `SplitView` actions (`openPane` / `setPaneConversation` / `reorderPanes`). Pointer/HTML5-DnD based; drag ghost + drop highlights use tokens (DEC-25).
- **ITEM-17**: Desktop **tear-off** — detect a drag that exits the app window bounds and, on drop, spawn a native `WebviewWindow` at the cursor (desktop only) by reusing ITEM-P1's `openConversationWindow`; on web this path is absent (drag-to-split + the explicit "New window/tab" button only). Bridges Phase 1 (pop-out) and Phase 2 (split). (DEC-25)
- **ITEM-18**: Right-panel **slide-over in split** — the `rightPanel` state (`tabs`/`activeId`/`panelWidth`/`mobileDrawerOpen`) is already per-pane (it's part of `ChatPaneStore`), so each pane owns its own panel for free. The only change is placement: when >1 pane is open, `ChatRightPanel` renders as a per-pane slide-over **absolute-positioned inside its own pane container** (a variant of the existing mobile full-cover overlay path — `absolute inset` scoped to the pane, focus-trap scoped to the pane), rather than an inline 3rd column; single-pane keeps today's inline resizable side panel unchanged (no regression). The module-level `panelRendererRegistry` (type → component) stays GLOBAL — a stateless descriptor like the `ExtensionCatalog`, registered once at catalog build, not per-pane; only the per-tab `data` is per-pane. `displayInRightPanel` called from a pane's subtree resolves `useChatPane()` → that pane's panel. Per-conversation localStorage persistence (keyed by conversationId) is unchanged, so panel tabs survive a tear-off into a new window for free (DEC-26).
- **ITEM-19**: `McpComposer` per-pane + two latent-bug fixes (the deepest content-render gap). Move per-conversation fields into the pane runtime: `toolCalls`, `approvalDecisions`, `elicitationRequests`, `selectedServers`, `currentConversationId`, `currentProjectId`, `configModalVisible` (keep `conversationConfigs` global — it's conv-id-keyed). FIX (a): `setToolCallProgress(server, …)` is keyed by SERVER, so two panes running a tool on the same MCP server cross-bleed progress — re-key by `(server, message_id)` (the `mcpToolProgress` event carries `server` + `message_id` but NOT `tool_use_id`, so it correlates to the pane via its streaming message, not the tool id). FIX (b): the tool-**approval** action reads the global `approvalDecisions` array + calls the singleton `Stores.Chat.sendMessage()` → approving in pane B posts to pane A's conversation; route it through the PANE's `sendMessage` + the pane's decisions. `McpToolUseRenderer`'s `tool_result` lookup in `Stores.Chat.messages` and `FileAttachmentRenderer.openInRightPanel` must resolve the PANE's messages / right panel. Elicitation/`ask_user` answer by global `elicitation_id` (`POST /mcp/elicitation/{id}/respond`) so they're pane-portable — only their render/block-injection needs pane-scoping.
- **ITEM-20**: Backend caveats (the only server changes — small). (a) Make `PER_USER_MAX_CONNECTIONS` (chat-stream `registry.rs:26`, currently a hardcoded `12`) configurable + raise the default, since one-connection-per-pane makes ~12 panes/user the ceiling and reconnect churn can 429 a legit pane (DEC-34); optionally let the SSE stream survive an access-token refresh without a full reconnect to cut churn. (b) No raw-event envelope change is required IF the frontend obeys the strict rule (ITEM-6): one dedicated connection per pane that is torn down + re-opened (never repointed) on a pane conversation switch. Backend approval/elicitation response routing is verified isolated — no change.
- **ITEM-21**: `MessageViewState` reset-scoping (the missed 6th per-conversation store). Its maps (`collapsed` by msgId, `files` by URI) are globally-unique-keyed so the DATA is pane-safe — the only bug is `resetViewState()` being a GLOBAL nuke fired on ONE pane's conversation switch (`Chat.store.ts:730`) + `__destroy__` (`:2154`) — the `reset()` action does NOT call it — wiping the other pane's collapse/seen/height state. Fix = reset-scoping only (NOT a full per-pane store): re-key both maps by conversationId and make `resetViewState(convId)` clear only that conversation's sub-map (pass the OUTGOING conv id at the two sites), threading convId into the `CollapsibleBlock`/`InlineFilePreview` selectors; or the minimal alt — drop the two reset calls (the author's own comment says reset is "not required for isolation", keys are unique). Also add the run_js chat surface to the enumerations (ITEM-19): SSE `runJsApprovalRequired` + content-type `run_js_approval` (both already inside the `mcp` extension → covered by ITEM-19's migration; no new extension/store).
- **ITEM-22**: store-kit — expose the raw `StoreApi` from a per-pane store. `ctx.chatStore` needs `subscribe`/`getState`/`setState` (8 `useChatStore.subscribe` sites + the `sseEventHandlers(data,get,set)` rebinding depend on it), but `defineLocalStore.use()` returns ONLY the read-proxy — the raw `api` is private. Either extend store-kit to surface the `StoreApi`, or have `ChatPaneProvider` build the store via `createStore` directly (bypassing `.use()`). This primitive underpins the entire ITEM-5 extension-migration contract, so it lands before ITEM-2/3/5.
- **ITEM-23**: Gallery/harness multi-pane support (its own reviewed change, per B3 — NOT a silent workaround). The gallery cassette (`mockApi.ts`) serves one global SSE stream, and the fixtures (`deepStates.tsx`/`seededSurfaces.tsx`/`shard5.tsx`) drive the singleton `useChatStore` — both are SHARED harness. To render two independent panes (one seeded via the `SplitView` store, both able to stream distinct content), `mockApi.ts` must key its SSE cassette by conversation/subscription and the fixtures migrate to per-pane seeds. This is a genuine multi-pane capability the harness lacks, so it lands as a reviewed infra change (B3: don't edit the shared harness to route around a feature bug — this is a real capability gap, documented here so it isn't a silent workaround).
- **ITEM-15**: Gallery + state-matrix coverage for the split surface — add gallery cells (loaded / empty / streaming, focused vs unfocused pane, and the mobile tab mode) so `check:state-matrix` / `check:gallery-coverage` and the runtime-health pass (`npm run gate:ui`) cover the new states, mirroring existing chat gallery seeds.

## Files to touch

New files (frontend, `src-app/ui/src` unless noted):
- `src-app/ui/src/modules/chat/core/popout/openConversationWindow.ts` — ITEM-P1 (platform pop-out util; Tauri `WebviewWindow` vs `window.open`).
- `src-app/ui/src/modules/chat/components/OpenInNewWindowAction.tsx` — ITEM-P2 (header/row-menu affordance).
- `src-app/ui/src/modules/chat/core/stores/SplitView.store.ts` — ITEM-1.
- `src-app/ui/src/modules/chat/core/pane/ChatPaneContext.tsx` — ITEM-3 (`ChatPaneProvider` + `useChatPane`).
- `src-app/ui/src/modules/chat/core/extensions/catalog.ts` — ITEM-4 (global `ExtensionCatalog`, extracted from `registry.tsx`).
- `src-app/ui/src/modules/chat/core/extensions/PaneExtensionRuntime.ts` — ITEM-4 (per-pane runtime).
- `src-app/ui/src/modules/chat/components/ChatPane.tsx` — ITEM-7.
- `src-app/ui/src/modules/chat/components/SplitChatView.tsx` — ITEM-8 (tab-strip workspace chrome).
- `src-app/ui/src/modules/chat/components/PaneTabStrip.tsx` — ITEM-8 (the pane-tab strip).
- `src-app/ui/src/modules/chat/core/pane/paneDnd.ts` + `src-app/ui/src/modules/chat/components/PaneDropZones.tsx` — ITEM-16 (drag/drop layer + edge drop-zones).
- `src-app/ui/src/modules/chat/core/popout/tearOff.ts` — ITEM-17 (desktop drag-past-window-bounds → new window).
- `src-app/ui/src/modules/chat/core/pane/paneStreamBridge.ts` — ITEM-6 (per-pane client wiring) if not inlined into the provider.
- `src-app/ui/src/modules/chat/core/stores/chatBridge.ts` — ITEM-9 (`Stores.Chat` focused-pane forwarder).
- `src-app/ui/src/modules/chat/core/split/limits.ts` — ITEM-14.
- Gallery seed additions under `src-app/ui/src/dev/gallery/` (+ desktop mirror) — ITEM-15.

New files (v2 redesign):
- `src-app/ui/src/modules/chat/core/split/reconcile.ts` — ITEM-25 (the pure `openConversationInWorkspace(convId, intent)` reconciliation reducer; the single rule every entry point routes through).
- `src-app/ui/src/modules/chat/core/stores/splitWorkspace.persist.ts` — ITEM-26 (per-user localStorage load/save + hydrate/prune + v1→v2 migration).
- `src-app/ui/src/modules/chat/components/ConversationPickerPane.tsx` — ITEM-27 (empty-pane searchable picker + "start a new chat").

Edited files:
- `src-app/ui/src/modules/chat/core/stores/Chat.store.ts` — ITEM-2 (defineStore → defineLocalStore def; relocate the auth-driven stream wiring to ITEM-6).
- `src-app/ui/src/modules/chat/core/stream/ChatStreamClient.ts` — ITEM-6 (singleton → factory).
- `src-app/ui/src/modules/chat/core/extensions/registry.tsx` — ITEM-4 (split into catalog + runtime; keep `register()` populating the catalog).
- `src-app/ui/src/modules/chat/core/extensions/index.ts`, `slots.tsx`, `types.ts` — ITEM-4/5 (export catalog + runtime; extend the extension `initialize`/hook signatures with `ctx`).
- `src-app/ui/src/modules/chat/pages/ConversationPage.tsx`, `NewChatPage.tsx` — ITEM-7/8.
- `src-app/ui/src/modules/chat/module.tsx` — ITEM-1/8 (register `SplitView` store; route → `SplitChatView`).
- `src-app/ui/src/modules/chat/types.ts` — ITEM-9 (`Stores.Chat` bridge type; add `SplitView`).
- Chat view components: `ChatInput.tsx`, `MessageList.tsx`, `ChatMessage.tsx`, `BranchNavigator.tsx`, `EditingMessageBanner.tsx`, `MessageActions.tsx`, `TextContent.tsx`, `TitleEditor.tsx`, `core/components/ChatRightPanel.tsx` — ITEM-10.
- Chat built-in extensions: `extensions/text/**`, `extensions/export/extension.tsx`, `extensions/keyboard/**`, `extensions/title/**`, `extensions/syntax/**` — ITEM-5/10.
- Other-module chat extensions: `src-app/ui/src/modules/file/chat-extension/**`, `mcp/chat-extension/**`, `user-llm-providers/chat-extension/**`, `assistant/chat-extension/**`, `memory/chat-extension/**`, `summarization/chat-extension/**`, `literature/components/**`, `skill/chat-extension/**`, `projects/chat-extension/extension.tsx` (+ `projects/pages/ProjectDetailPage.tsx`, `projects/pages/ProjectChatPage`) — ITEM-5/10/13.
- `src-app/ui/src/modules/mcp/stores/McpComposer.store.ts` + `src-app/ui/src/modules/mcp/chat-extension/**` — ITEM-19 (per-pane MCP composer + progress-keying + approval-routing fixes).
- `src-app/server/src/modules/chat/stream/registry.rs` + the deployment config plumbing — ITEM-20 (configurable connection cap). This is the ONLY backend touch; it makes the diff back-end+front-end, so phase-8 runs the backend integration chain too (a `registry.rs` unit/integration test), no migration/OpenAPI.
- Desktop mirror: `src-app/desktop/ui/src/modules/host-mount/conversation-extension/components/ConversationMountsControl.tsx` — becomes pane-scoped via `useChatPane()` (renders in the per-pane header slot), NOT the bridge; verified via desktop `npm run check`. Desktop reuses the SAME chat sources (vite `@/` alias → `ui/src`), so there are no separate desktop extension copies — the refactor lands once in `ui/src`.

Edited files (v2 redesign):
- `src-app/ui/src/modules/chat/core/stores/SplitView.store.ts` — ITEM-24/25/26 (workspace store: stable ids, one-conv-per-pane, `openConversationInWorkspace`, persistence hookup; DROP `?pane=` URL mirroring).
- `src-app/ui/src/modules/chat/components/SplitChatView.tsx` — ITEM-24/29/30 (workspace-driven pane list; columns vs tab-strip `mode`; close/pop-out/max-pane semantics).
- `src-app/ui/src/modules/chat/pages/ConversationPage.tsx` — ITEM-25 (route the URL `:conversationId` through the reconciliation reducer against the hydrated workspace; URL = focused pane).
- `src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx` + `src-app/ui/src/modules/chat/components/ConversationCard.tsx` — ITEM-28 (clicks → reconciliation not raw `navigate`; modifier/middle-click → new pane; add "Open in split pane" ⋯-menu item; drag source for ITEM-31).
- `src-app/ui/src/modules/chat/core/pane/paneDnd.ts` + `PaneDropZones.tsx` — ITEM-31 (implement the deferred drag-drop; drop-on-pane=replace, drop-on-seam=new pane, header-drag=reorder).
- `src-app/ui/src/modules/chat/components/PaneTabStrip.tsx` — ITEM-30 (build the deferred mobile tab-strip mode).

## Patterns to follow

- **Pop-out util (ITEM-P1..P4):** mirror the existing `window.__TAURI__` platform
  branch used across desktop-aware components (e.g. the web client's
  `!window.__TAURI__` refresh-cookie gate in `Auth.store`), and the desktop
  `desktop-base/module.tsx` `invoke('auto_login')` boot; use Tauri 2.8's
  `@tauri-apps/api/webviewWindow` `WebviewWindow` for native windows (shell-native
  → Tauri API, per `[[feedback_no_tauri_ipc]]`'s shell-native exception). Reuse
  the conversation-list row-menu + header-action slot patterns for the affordance.
- **Per-pane store (ITEM-2/3):** mirror `modules/llm-provider/widgets/LLMProviderGroupWidget.store.ts` + `.tsx` (the only production `defineLocalStore`: `.use({key})` per row, `init` guarding on the instance's own key) and `core/store-kit.ts` (`defineLocalStore`, `LocalStoreInstance`, isolated `local:<n>` groups).
- **Global layout store (ITEM-1):** mirror `modules/chat/stores/ChatHistory.store.ts` (`defineStore`, `immer`, `init: ({on, set, actions})`) and the app-layout store for panel widths (`modules/layouts/app-layout/`).
- **Pane context (ITEM-3):** mirror the small chat contexts already present — `modules/chat/components/PlusDropdownContext.ts` and `modules/chat/core/MessageContext.tsx` — for the provider/`useX()` shape.
- **Extension catalog vs runtime (ITEM-4/5):** preserve the existing `ChatExtensionRegistry` shape in `core/extensions/registry.tsx`; keep its registration maps as the catalog, extract the store-instance + lifecycle half into the per-pane runtime. Extension stores keep using `defineExtensionStore` (`core/store-kit.ts`).
- **Per-pane stream client (ITEM-6):** mirror `core/sync/SyncClient.ts` (start/stop, backoff, epoch, ReadableStream SSE parse) — `ChatStreamClient.ts` is already modeled on it; convert its module-level singleton state into closure state returned by a factory. Keep the once-wired auth lifecycle pattern (`core/sync/index.ts`).
- **Divider (ITEM-8):** reuse `modules/layouts/app-layout/components/ResizeHandle.tsx` exactly as `ChatRightPanel.tsx` does (it supports `placement='left'|'right'` vertical dividers, `role="separator"`, keyboard, min/max clamps, `onEnd` commit).
- **Pane view extraction (ITEM-7):** extract verbatim from `pages/ConversationPage.tsx` (scroll-anchor effects, header, slots) — preserve the `conversation?.id === conversationId` scroll-latch invariants.
- **Bridge proxy (ITEM-9):** mirror `core/stores.ts` `createStoreProxy` / the `Stores` proxy (snapshot via `.$`, actions returned directly, reactive reads via `useStore`).
- **Mobile gating (ITEM-12):** mirror `ConversationPage.tsx` / `ChatRightPanel.tsx` use of `useWindowMinSize` + `useNativeScroll` / `Stores.AppLayout.nativeScroll`.
- **Gallery (ITEM-15):** mirror the existing chat gallery seeds and `dev/gallery/seeded/shard3.tsx` (the one gallery `defineLocalStore` usage) + the state-matrix conventions in `DESIGN_SYSTEM.md` / the UI Build Gate.
- **Reconciliation reducer (ITEM-25):** keep it a PURE function `(workspaceState, convId, intent) → newState` (unit-testable in isolation, mirroring `SplitView.store.test.ts`'s reducer tests), invoked by the store action + the router effect — not scattered `navigate()` calls.
- **Per-user localStorage persistence (ITEM-26):** mirror `modules/chat/extensions/text/chatDrafts.ts` (`makeDraftKey(userId, …)` namespacing) + the existing `ziee-split-view-v1` load/save in `SplitView.store` for the read/write/versioning shape.
- **Empty-pane picker (ITEM-27):** mirror the existing searchable conversation list — `modules/chat/components/ConversationList.tsx` / the `RecentConversationsWidget` row rendering + the kit search input — reuse its row component, don't rebuild.
- **Sidebar reroute + ⋯-menu item (ITEM-28):** mirror the existing `RecentConversationsWidget` row-menu (the Delete item + `keepMenuOpen`) for the new "Open in split pane" entry; mirror `ConversationCard`'s `navigate(href)` site for the reroute-through-reconciliation swap.
- **Tab strip (ITEM-30):** mirror the kit `Tabs` (`@/components/ui`, `data-slot="tabs-*"`) already used by `ChatRightPanel`'s tab list — same trigger/close-button shape.
- **Drag-and-drop (ITEM-31):** mirror the existing pointer-drag in `SplitChatView`'s `SplitDivider` (pointer capture + move/up handlers) for the low-level drag; for reorder/drop-index, follow the closest in-repo list-reorder pattern (`shadcn-component-discovery` first — reuse a kit primitive over a bespoke dnd lib).
