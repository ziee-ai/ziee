# ISOLATION_AUDIT — split-chat per-pane state-sharing (multi-agent, converged)

Exhaustive audit of the chat surface for split-pane state leaks. 8-area sweep +
completeness critics, converged at 6 rounds (2 dry). 79 raw findings → 34
distinct issues in 10 owner groups. Full per-agent findings: workflow
`wf_9b6c4c04-b8c` journal. Verdict: reuse-the-engine-unchanged is NOT tenable;
fixes are targeted per-pane refactors (ITEM-32..39), not a rewrite.

# Split-Chat Per-Pane State Consolidation — 79 findings → 34 distinct issues in 10 owner groups

Dedup collapsed the 79 raw findings (multiple audit passes re-reported the same store from different call sites) to **34 distinct issues**. Grouped below by **owning store/surface**, ranked by max severity. `planned=true` flags in the input are inconsistent — I've corrected them where an issue is *not* actually inside ITEM-32's scope (composer TextStore + `Stores.File` only). New ITEM-33..39 proposed for everything ITEM-32 misses.

---

## Three cross-cutting root causes (every group is one of these)

1. **Global singleton store that must become per-pane / pane-keyed** — `Stores.File`, `Stores.McpComposer`, `ConversationSummarization`, `MessageViewState`, right-panel persistence, `NEW_CHAT_*` sentinel keys.
2. **Bridge mis-resolution to the *focused* pane** — a per-pane component reads `Stores.Chat.$` (snapshot) or calls `Stores.Chat.<action>` (action); both route through `chatBridge`→`focusedApi()`, i.e. the pane with focus, **not** the render/sending pane. Masked by `SplitChatView`'s `onPointerDownCapture` (mouse focuses the render pane first) but **leaks on keyboard activation, after an `await`, or on timer callbacks**. Uniform fix: rebind to `useChatPane().store` / capture the pane store before the await / thread the originating pane into the hook.
3. **Process-global side-channel** — module singletons (`capturedDraftKey`, `inPlaceAnchorSignal`, `globalKeyboardHandler`), document-order DOM lookups (`querySelector`/`getElementById`), per-pane-registered `window`/`document` listeners that all fire, one global `EventBus` every pane subscribes to, and `window.location`. Fix: key-by-paneId / scope-to-pane-subtree / tag-frames-by-client.

---

## HIGH-severity groups

### G1 — `Stores.File` composer buffer → **ITEM-32 (planned)**
Raw: `File.store.ts:268/272/273/274/275`, `file/chat-extension/extension.tsx:170/320/351`. Distinct: `selectedFiles`, `uploadingFiles`, `restoredFileIds`, `backupSelected/UploadingFiles` are a single global buffer; the file extension's `composeRequestFields`/`provideUserContent`/`onMessageSent` read+clear it; its `initialize()` subscribes only to the **primary** pane.
**Symptom:** attaching/uploading in pane A shows in pane B's tray and gates B's Send; A's send clears B's attachments and carries A's files; a primary-pane conversation switch wipes the shared buffer under whatever pane is attaching; a non-primary pane never auto-clears on switch or restores on edit.
**Fix owner:** ITEM-32 (this is exactly its remit).

### G2 — Text composer TextStore + send-path hooks + draft persistence → **ITEM-32 (state) + ITEM-34 (hook threading) + ITEM-39 (module var)**
Raw: `text/extension.tsx:18/131/206`, `chatDrafts.ts:55`, `CanvasSelectionPopover.tsx:66`.
**Symptom:** ITEM-32 makes `TextStore` per-pane, but the text extension's `composeRequestFields`/`beforeSendMessage`/`onMessageSent`/`onStreamError`/`afterStreamComplete` still read `Stores.Chat.$.TextStore` = *focused* pane, so a pane-A stream error runs `restoreFromBackup()` into **pane B's** composer and clears the wrong backup. `capturedDraftKey` (module var) races on concurrent sends and clears the wrong conversation's draft. `chatDrafts` localStorage key is `user:conversation` only, so two panes on the same conversation last-write-wins each other's persisted draft. Canvas "Ask/Edit this" injects into the focused pane's TextStore.
**Fix owner:** hook pane-threading → ITEM-34; `capturedDraftKey` → ITEM-39; `chatDrafts` key + canvas → ITEM-32 residual (call `useChatPane().store.$.TextStore`, key draft by pane for same-conversation splits).

### G3 — `Stores.McpComposer` (+ Skill drawer, + config-modal) → **NEW ITEM-33** *(mis-tagged `planned=true` in input — ITEM-32 does NOT cover this store)*
Raw: `McpComposer.store.ts:124/125/127/128/129/132`, `mcp/chat-extension/extension.tsx:352/394/882/915/1036/1047`, `McpStatusRow.tsx:13`, `ToolCallPendingApprovalContent.tsx:59/64`, `SkillConversationDrawer.store.ts:6`.
**Symptom:** `selectedServers`, `currentConversationId/ProjectId`, `approvalDecisions`, and `toolCalls` are all single-global. Whichever pane last ran `onConversationLoad→setCurrentConversation` owns the shared selection → the *other* pane sends with the wrong conversation's MCP servers; `approvalDecisions` is one array so approving a tool in pane B queues a decision pane A's next send transmits, and either send clears both; `setToolCallProgress(server,…)` attaches a progress frame to the in-flight call in **both** panes streaming the same server. `configModalVisible` (and Skill drawer `open`) are single booleans while the modal host renders once **per pane** → opening MCP/Skill config in B opens the Dialog in **both** panes.
**Fix owner:** NEW ITEM-33 — key McpComposer state by pane; per-pane modal-open flags.

### G4 — Chat extension-registry runtime + hook pane-threading + edit-restore subscriptions → **NEW ITEM-34**
Raw: `registry.tsx:23/375`, `Chat.store.ts:719/1992/2201`, `{model,assistant,mcp,file}/…/extension.tsx:41/50/170/177/352`, plus the `Stores.Chat.$`-reading hooks in G2/G3.
**Symptom:** the registry is a process-wide singleton with one `initialized` flag and one global `cleanup()`. Any pane's `loadConversation`(switch)/`reset()`/**store `onCleanup` on pane close** runs *every* extension's cleanup — tearing down the keyboard listener + file subscriptions for **surviving** panes — and `initialized=false` is left with **no re-`initialize()`** on the pane-close path, permanently killing Ctrl+Enter/Ctrl+K/Esc + file-clear-on-switch until some pane next switches conversation. Edit-restore `editingMessage` subscriptions (model/assistant/mcp/file) bind to the **primary** pane only (editing in a non-primary pane restores nothing) and are never unsubscribed, so each cleanup+init cycle **stacks duplicate** subscribers → K duplicate `getAssistant`/`getModel`/`getMcpServers` calls with last-write-wins races. Lifecycle hooks read `Stores.Chat.$` (focused pane) instead of the sending pane.
**Fix owner:** NEW ITEM-34 — per-pane registry instances (or paneId-keyed state), pass the originating pane's store into every hook, bind edit-restore to each pane's own store with captured unsubscribes.

### G5 — Streaming EventBus fan-out (same-conversation splits) + scroll anchor → **NEW ITEM-35**
Raw: `Chat.store.ts:2154/2167`, `ChatStreamClient.ts:199/216`, `useInPlaceAnchor.ts:22`.
**Symptom:** each pane owns its own `ChatStreamClient` (ITEM-6) but every client re-emits frames onto the **one** global `EventBus`, every pane store subscribes, and `applyStreamFrame` filters only by `conversation.id` — not by originating client. When two panes hold the **same** `conversation.id` (the flagship "compare two branches side-by-side" case — branches share one conversation.id), both panes process each frame and `text = currentText + delta` with no dedup → **live text is doubled/tripled and garbled in both panes for the whole generation** (self-heals only on `complete`'s `reconcileTail`). `chat:stream-reconnect` carries no pane id, so mounting/reconnecting one pane triggers **every** other pane's `resyncOpen()` refetch. `inPlaceAnchorSignal` is a single module `{key}` whose own comment asserts "only one MessageList mounted" — false in split; an in-place expand in A clobbers the parked key → the ~1300px scroll teleport reappears in the other pane.
**Fix owner:** NEW ITEM-35 — tag frames with pane/connection id and accept only own-client frames; per-pane `inPlaceAnchorSignal`. **This is the strongest evidence against reusing the v1 engine unchanged.**

### G6 — Right panel: state + actions + persistence → **NEW ITEM-36**
Raw: `LiteratureScreeningPanel.tsx:35/38/56`, `ChatRightPanel.tsx:86/88/143/145/166`, `InlineFilePreview.tsx:156` (+`AttachedFileCard`/`FilePreviewList`/`LiteratureToolResultCard`), `Chat.store.ts:240` (`PANEL_STORAGE_KEY`), `File.store.ts:291`.
**Symptom (HIGH — real data loss):** literature `persist()`/`flushReason()` read `Stores.Chat.$.rightPanel` and call `updateRightPanelTab` on the **focused** pane; blurring pane B's exclusion-reason input by clicking pane A focuses A first, so the write targets A (which lacks that session tab → silent `findIndex===-1` no-op) and **the typed reason is dropped**. Right-panel tab actions (`setActiveRightPanelTab`/`closeRightPanelTab`/`closeAllRightPanelTabs`), `displayInRightPanel`, and the Escape→`closeMobileDrawer` handler all route to the focused pane → keyboard-activating B's tab strip closes/switches **A's** tabs (close-all is destructive on A). Panel persistence is one localStorage slot keyed by conversation id, not pane → a second pane on the same conversation inherits the first's tabs on mount, and one pane's close-all `delete all[X]` wipes the other's persisted panel. `Stores.File` file-viewer view-state (`fileViewModes`/`imageViewStates`/`fileFindOpen`/`fileWordWrap`/`fileTabularView`) is file-id-keyed with no pane dimension → same file open in both panels mirrors raw/compiled/zoom/find/wrap.
**Fix owner:** NEW ITEM-36 — bind renderers+actions to `useChatPane().store`, key panel persistence by (conversation,pane) or keep in-memory-per-pane, move file-viewer view-state off the shared store.

### G7 — Header / chrome → **NEW ITEM-37** *(AssistantStatusChip new-chat key mis-tagged `planned=true`; ITEM-32 does not cover the sentinel-key collision)*
Raw: `ConversationSummarization.store.ts:11`, `ConversationFindBar.tsx:70`, `ConversationPage.tsx:272/449`, `TitleEditor.tsx:51`, `EditingMessageBanner.tsx:36`, `AssistantStatusChip.tsx:14`, `ModelPicker.store.ts:14/35`, `AssistantPicker.store.ts:10/26`, `RecentConversationsWidget.tsx:97`.
**Symptom (HIGH):** `ConversationSummarization` is a single-entry global (`current`) that both panes' status pills overwrite on conv-switch/`messages.size` change (`loadForConversation` drops `current` on id mismatch), so the in-thread "earlier N messages condensed" marker flips to whichever pane last streamed and a new-chat pane's `.clear()` wipes the other pane's marker. **Medium:** find-bar `activateMatch` (timer path) and Cmd-F/`#message-<id>` listeners are registered per pane and resolve via focused pane / single global URL → one Cmd+F opens find in **every** pane, one deep-link hash jumps every pane on that conversation, and the debounced initial match jumps the wrong pane. Title-edit Save and editing-banner Cancel are focused-pane actions (keyboard leak → rename/cancel the wrong pane). Model/Assistant pickers are conversation-keyed (safe) **except** the shared `NEW_CHAT_MODEL_KEY`/`NEW_CHAT_ASSISTANT_KEY='__new_chat__'` → two new-chat panes share one model/assistant selection. **Low:** sidebar highlight derives from one URL, so only the routed pane's conversation is highlighted (answers "one URL or all panes?" — it's one URL).
**Fix owner:** NEW ITEM-37 — per-pane/keyed summarization read-model, scope find/deep-link/title/banner to the owning pane, namespace new-chat sentinel keys per pane.

### G8 — Message-render: message-list actions + tool-result renderers → **NEW ITEM-38**
Raw: `WorkflowWorkspaceRunCard.tsx:32`, `MessageActions.tsx:56/72`, `BranchNavigator.tsx:36`, `ChatInput.tsx:29`, `ToolCallPendingApprovalContent.tsx:64/119` (post-await error read).
**Symptom (HIGH):** `WorkflowWorkspaceRunCard` reads `Stores.Chat.$.conversation?.id` at render (focused pane) and closes it over `onSave`/`onDownload` → "Save to my workflows"/"Download .tar.gz" export the **other** pane's conversation workspace. **Medium:** `MessageActions` Edit/Regenerate and `BranchNavigator` prev/next are focused-pane actions; on same-conversation splits (shared conversation.id) keyboard activation **regenerate auto-sends a turn on the wrong pane** and `activateBranch` stamps B's branchId onto A's conversation then `loadMessages` replaces A's window — corrupting A. `ChatInput` captures `sendMessage` from the focused-pane bridge at render, so B's Send button can fire on A. The MCP-approval card re-reads `Stores.Chat.$.error` after the await → wrong-pane error check reverts the approval panel spuriously.
**Fix owner:** NEW ITEM-38 — rebind these components/renderers to `useChatPane().store`, capture the pane store before any await.

### G9 — Module singletons / global-DOM handlers → **NEW ITEM-39**
Raw: `keyboard/extension.tsx:31/70/129`, `useStreamdownComponents.tsx:116`, `projects/chat-extension/extension.tsx:136`.
**Symptom (HIGH):** the keyboard extension registers **one** app-wide `document` keydown listener whose actions resolve the composer via `document.querySelector('button[aria-label="Send message"]')` / `textarea[placeholder*="Type your message"]` → **first DOM match = leftmost pane**, so Ctrl+Enter always sends pane A, Ctrl+K focuses A's textarea, Esc clears A — regardless of which pane you're typing in. **Medium:** markdown footnote/heading anchors scroll via `document.getElementById` (document-order-global) → on same-conversation splits, clicking a `[1]` in pane B scrolls pane A. `projects.afterCreateConversation` derives the target project from `window.location.pathname`, not the sending pane → sending in a plain pane B while the URL sits on project A files **B's new conversation into project A**.
**Fix owner:** NEW ITEM-39 — replace document-order resolution with per-pane-scoped handling; derive project-attach from the sending pane.

---

## LOW–MEDIUM group

### G10 — Message-render shared view-state → fold into **ITEM-38**
Raw: `CollapsibleBlock.tsx:68`, `InlineFilePreview.tsx:151/199/223`, `MessageViewState.store.ts:26`, `Chat.store.ts:746`, `MessageList.tsx:303`.
**Symptom:** `MessageViewState` is global but message-id-keyed (safe across *different* conversations); on the **same** conversation in both panes, expand/collapse/inline-file resize mirrors. **Real bug (LOW but a defect):** `Chat.store.ts:746` `resetViewState(Array.from(get().messages.keys()))` runs **after** line 734 already emptied `messages`, so it passes `[]` and deletes nothing — the outgoing conversation's collapse entries linger and a later pane inherits stale collapse state. DEV-only `window.__MSGLIST_METRICS__` is clobbered by the last-mounted pane and `delete`d when any pane unmounts (test-instrumentation collision only).
**Fix owner:** ITEM-38 — key MessageViewState reset correctly (read ids before clearing), key metrics/anchors by paneId.

---

## Proposed NEW ITEMs

| ITEM | Owner group | Max sev | Distinct issues | Core fix |
|---|---|---|---|---|
| **32** (exists) | `Stores.File` + TextStore composer | HIGH | 8 | per-pane composer store (already planned) |
| **33** | `Stores.McpComposer` + Skill drawer + config-modal | HIGH | 6 | pane-key MCP selection/approvals/toolcalls; per-pane modal flags |
| **34** | Extension-registry runtime + hooks + edit-restore subs | HIGH | 4 | per-pane registry; thread sending pane into hooks; per-pane subs |
| **35** | Streaming EventBus + scroll anchor | HIGH | 3 | tag frames by client/pane; own-frame filter; per-pane anchor |
| **36** | Right panel (state/actions/persistence/viewer) | HIGH | 5 | rebind to `useChatPane().store`; pane-scope persistence + viewer state |
| **37** | Header/chrome + new-chat keys | HIGH | 8 | per-pane summarization; scope find/deep-link/title/banner; namespace `__new_chat__` |
| **38** | Message-render actions + tool-result renderers + view-state | HIGH | 6 | rebind renderers/actions to owning pane; fix `resetViewState` no-op |
| **39** | Module singletons / global-DOM handlers | HIGH | 3 | per-pane-scope keyboard/markdown/project resolution |

*Correction to input flags:* the four `McpComposer` selection findings and the `AssistantStatusChip` new-chat-key finding are marked `alreadyPlanned=true` but fall **outside** ITEM-32's stated scope (composer TextStore + `Stores.File`); they require ITEM-33 / ITEM-37 respectively. Conversely, `File.store.ts:291` (file-viewer view-state, `planned=false`) is only *nominally* inside `Stores.File` — it is a right-panel viewer concern and is assigned to ITEM-36.

---

## Verdict — "reuse the v1 engine unchanged" is **not tenable**

The v1 chat engine is architecturally single-conversation: it leans on process-global singleton stores (`Stores.File`, `McpComposer`, `ConversationSummarization`, `MessageViewState`), a single-instance extension registry with an `initialized` flag and one global `cleanup()` that any pane's lifecycle can fire (killing surviving panes' keyboard + subscriptions), module-level singletons (`capturedDraftKey`, `inPlaceAnchorSignal`, `globalKeyboardHandler`), document-order DOM resolution, a `chatBridge` that silently resolves reads/actions to the *focused* pane, and — most damning — a **single global EventBus that every pane subscribes to with only conversation-id filtering**, so the flagship "compare two branches side-by-side" case (branches share one `conversation.id`) **double-processes every streamed frame and garbles live text in both panes**. None of these are cosmetic; they corrupt sends, edits, exports, and the core streaming path. That said, the fixes are **targeted per-pane refactors (ITEM-32..39), not a rewrite**: the pane infrastructure (per-pane `ChatStreamClient`, `useChatPane()`, `SplitChatView` focus) already exists, and every issue reduces to one of three mechanical sweeps — make the store per-pane/pane-keyed, rebind bridge reads/actions to `useChatPane().store`, or key the global side-channel by paneId. So: reuse the engine's *shape*, but ship ITEM-32 through ITEM-39 before split-chat is correct — "unchanged" would ship a demonstrably corrupting UI, especially for same-conversation splits.
