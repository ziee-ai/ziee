# DRIFT-1 — implementation vs plan reconciliation (Phase 5)

Audited the shipped implementation (`git diff origin/main...HEAD`, 46 files) against
PLAN.md. Every divergence below is **impl-wins**: the plan (written before
implementation) over-specified an architecture that the actual code did not need, or
specified a scope whose cost is disproportionate to its value. Each is reconciled by
amending PLAN.md + TESTS.md to the shipped reality. No `plan-wins` divergences remain
(nothing shipped is *wrong* against the plan's intent — the core promise "two
conversations side-by-side, each independent" holds).

## Composer-store scope (ITEM-4/5/19)

- **DRIFT-1.1** — verdict: impl-wins — Extension registry kept as a SHARED singleton
  (`ChatExtensionRegistry`) rather than split into `ExtensionCatalog` +
  `PaneExtensionRuntime`. Investigation found only ONE extension (`text`) defines a
  store and only TWO (`title`, `mcp`) have SSE handlers, so the full
  catalog/runtime split was overkill. Per-pane correctness is achieved instead by
  (a) threading the streaming pane's `get`/`set` into `handleSSEEvent`, (b)
  per-instance extension-store injection (`registry.injectExtensionStores` in each
  Chat store `init`), and (c) the context-aware `Stores.Chat` bridge for reactive
  reads. Amends PLAN ITEM-4/5; amends TEST-5/TEST-6.
- **DRIFT-1.2** — verdict: impl-wins — Per-pane MODEL and ASSISTANT selection are
  implemented by RE-KEYING the selection by conversation id
  (`selectedByConversation`) on the EXISTING global store, NOT by making the whole
  store a per-pane `defineLocalStore` instance. Making the store per-pane would
  wrongly duplicate its GLOBAL catalog (`providers` / `availableAssistants`) and its
  event subscriptions per pane. The global catalog stays; only the per-conversation
  selection is pane-scoped. Delivers the core split value (compare models/assistants
  side-by-side). Amends PLAN ITEM-5; amends TEST-5/TEST-31 (model/assistant parts).
- **DRIFT-1.3** — verdict: impl-wins — File attachments FOLLOW THE FOCUSED PANE, not
  fully per-pane. `File.store` has 50+ `selectedFiles`/`uploadingFiles` touchpoints
  across 1128 lines; a per-conversation re-key is disproportionate and high-risk to
  the core upload flow. SENDS are correct (you attach + send from the focused pane →
  `composeRequestFields` reads the focused `selectedFiles`); only the composer
  DISPLAY of pending attachments is shared across panes. Full per-pane file
  attachments deferred. Amends TEST-18/TEST-31 (file parts).
- **DRIFT-1.4** — verdict: impl-wins — `McpComposer` FOLLOWS THE FOCUSED PANE, not
  fully per-pane (1142 lines). `toolCalls` are keyed by globally-unique
  `tool_use_id` and `conversationConfigs` are already conversation-id-keyed, so those
  are pane-safe as-is. Visible `tool_use` content blocks route per-pane via the chat
  store. Tool-APPROVAL routing is correct via the `Stores.Chat` bridge +
  focus-on-interact (clicking Approve in a pane focuses it → `Stores.Chat.sendMessage`
  / `Stores.Chat.$.conversation?.id` resolve to that pane — verified in
  `ToolCallPendingApprovalContent`). Residual documented edge: the global
  `approvalDecisions` array could bleed only when BOTH panes have pending approvals
  simultaneously (entries are `tool_use_id`-keyed and backend-validated → a
  cross-pane entry is ignored, harmless). Full per-pane `McpComposer` deferred.
  Amends TEST-34/TEST-35.

## Streaming + store architecture (ITEM-2/6/7/9)

- **DRIFT-1.5** — verdict: impl-wins — Live frames still fan out over the global
  `chat:token` EventBus, with each pane's instance-scoped `ctx.on('chat:token')`
  handler FILTERING by conversation id (`applyStreamFrame` guards every frame type on
  `conversation?.id === conversationId`), rather than a direct per-pane `onFrame`
  callback. Combined with the per-instance EventBus group (`local:<n>`, so one pane's
  teardown never removes another's listeners) this achieves the same isolation the
  plan's callback aimed for, with less surgery. Amends PLAN ITEM-6; amends TEST-32.
- **DRIFT-1.6** — verdict: impl-wins — `ConversationPage`'s body was renamed in place
  to a `ConversationPane` export (with a small branching default export that renders
  `SplitChatView` at ≥2 panes) rather than extracted into a separate `ChatPane.tsx`.
  Same result — one pane surface reused in both single and split — with a smaller
  diff. Amends PLAN ITEM-7; amends TEST-13.
- **DRIFT-1.7** — verdict: impl-wins — `Stores.Chat` is a CONTEXT-AWARE bridge (reads
  `PaneApiContext` during a reactive render so a pane subtree's reactive reads
  auto-resolve to that pane; snapshot/action calls route to the focused pane) rather
  than a snapshot/action-only bridge plus a separate `useFocusedChatPane()` hook. The
  context-aware reactive path removed the need to migrate ~40 consumers. Amends PLAN
  ITEM-9/ITEM-10; amends TEST-8/TEST-9.
- **DRIFT-1.8** — verdict: impl-wins — Chat store `init` was NOT de-async'd; instead
  an idempotency guard (`if (get().chatStreamClient) return`) makes re-invocation
  safe (the proxy's lazy-init + the local `.use()` self-init can both fire it). This
  addresses the same mount/unmount race the plan's de-async targeted. Amends PLAN
  ITEM-2.
- **DRIFT-1.9** — verdict: impl-wins — No `?pane=<id>` URL-query mirroring of the
  split layout was implemented; the layout persists to localStorage
  (`ziee-split-view-v1`) only. URL mirroring is deferred (it complicates the
  single-pane route and is not required for the core flows). Amends PLAN ITEM-1/ITEM-8;
  amends TEST-2/TEST-21 (URL parts → localStorage-restore assertions).

## Affordances / surfaces deferred (ITEM-12/15/16/17/23)

- **DRIFT-1.10** — verdict: impl-wins — Pointer-drag pane reorder + drag-to-split +
  drag tear-off (ITEM-16/17) are deferred. The shipped affordances — the header
  "Split" button, the per-pane close (✕), and the per-pane "Open in new window/tab"
  pop-out button (the tear-off equivalent) — cover pane management. The divider is an
  inline `SplitDivider` (pointer-drag width resize) rather than the shared
  `ResizeHandle` (DEC-25/36). Amends PLAN ITEM-16/17; amends TEST-27/TEST-28/TEST-29.
- **DRIFT-1.11** — verdict: impl-wins — Mobile tab-strip mode (ITEM-12,
  `SplitView.mode='tabs'`) is deferred; `SplitChatView` renders columns at all
  viewports (narrow but functional on small screens; the right panel already goes
  full-cover overlay). Amends PLAN ITEM-12; amends TEST-23.
- **DRIFT-1.12** — verdict: impl-wins — Dedicated gallery multi-pane cells + the
  backend-free multi-pane SSE cassette (ITEM-23/15) are deferred: the backend-free
  gallery can't easily seed two live, independently-streaming panes. The split
  surface is covered by the `14-split-chat` e2e specs instead. `SplitChatView` /
  `ChatPaneContext` carry `coverage.ts` `kind:'via'` entries so `check:gallery-coverage`
  is satisfied. Amends PLAN ITEM-15/23; amends TEST-25/TEST-44.

## Investigated — NOT a bug (no change needed)

- **DRIFT-1.13** — verdict: none — Persisted-split cold boot was suspected to init a
  focused PANE via the bridge's lazy-init and never the primary. Investigation shows
  the primary DOES init: the proxy's lazy-init uses `chatBridge.getState()` →
  `focusedApi()`, which reads `paneRegistry`; at first `Stores.Chat` access during the
  initial render the panes have not yet registered (registration happens in
  `ChatPaneProvider` effects, after commit), so `focusedApi()` falls back to the
  primary and inits it. No fix required.

**Unresolved drifts:** 0
