# PLAN_AUDIT — split-chat-multipane

Audited against the codebase at `origin/main` (worktree `feat/split-chat-multipane`).
This is the most safety-critical phase: the refactor touches the most-used and
most-recently-hardened surface (chat), so the emphasis is on *what breaks* and
*what conforms*.

## Breakage risk

The chat runtime has ~40 `Stores.Chat` consumers across chat core, the chat
built-in extensions, and 7 other modules' chat-extensions, plus 1 desktop
consumer. The migration strategy is **incremental via the ITEM-9 bridge**:
`Stores.Chat` keeps working (forwarding to the focused pane) so only the
pane-scoped subtree must move to `useChatPane()` in v1. Concrete risks found:

1. **`applyStreamFrame` cross-pane fan-out (correctness).** With N panes each
   subscribed to the shared `chat:token` EventBus, every pane store receives
   every pane's frames. The existing `get().conversation?.id === conversationId`
   guards already drop non-matching frames, so this is safe *provided two panes
   never hold the same conversation* — DEC-9 forbids duplicate conversations
   across panes. The module-level `frameApplyTail` serializer must become
   **per-pane** (each pane instance owns its tail) or fast frames across panes
   would serialize against each other and one pane's slow extension hook would
   stall another pane's tokens. Covered by ITEM-2/ITEM-6.

2. **New-chat-in-a-pane must not hijack global routing (correctness).**
   `conversation.created` is a GLOBAL EventBus event; `NewChatPage` turns it into
   `navigate('/chat/:id')`. A pane that lazily creates a conversation on first
   send would fire this and navigate the whole window. Fix: the creating pane
   adopts the returned conversation locally (`setPaneConversation`) and the
   global navigate is gated to the *primary/URL* pane only (or the created event
   carries the originating paneId). Covered by ITEM-1/ITEM-7/ITEM-11 — flagged
   so the drift loop verifies no cross-pane navigation regression.

3. **Keyboard shortcuts use global `document.querySelector` (correctness).** The
   keyboard extension resolves the send button via
   `button[aria-label="Send message"]` and the textarea via
   `textarea[placeholder*="Type your message"]` — with N panes there are N of
   each and `querySelector` returns the FIRST. Ctrl+Enter/Ctrl+K/Esc must scope
   to the **focused** pane's container. Covered by ITEM-5 (keyboard is in the
   migrate list) + the focused-pane concept (ITEM-9); the fix is to query within
   the focused pane's root element.

4. **Extensions that call `useChatStore.subscribe` cannot ride the bridge
   (state-management).** file / mcp / user-llm-providers / assistant register
   imperative `useChatStore.subscribe(...)` listeners in `initialize()` bound to
   the ONE singleton api. A focused-pane *forwarding* proxy cannot re-target a
   live `.subscribe`. These MUST move into the per-pane runtime so their
   `initialize` subscribes to THAT pane's store api. This is exactly ITEM-5 and
   is the single largest correctness surface of the refactor.

5. **The `Stores.Chat` reactive bridge across a changing focused instance
   (state-management).** Snapshot reads (`Stores.Chat.$.x`) and action calls
   forward trivially (read the focused pane's `getState()`). Reactive reads
   (`Stores.Chat.messages` in render) are harder: the underlying zustand api
   changes when focus moves panes. zustand's `useStore` is built on
   `useSyncExternalStore`, which tolerates a changing subscribe/getSnapshot, so a
   proxy that calls `useStore(focusedApi, sel)` each render works — but it must
   also react to `focusedPaneId` changes. Mitigation: keep the bridge
   **snapshot+action only**, and provide a dedicated `useFocusedChatPane()` hook
   for the *very few* genuinely out-of-subtree reactive reads. (SUPERSEDED detail:
   both originally-cited consumers actually migrate — export→`ctx` (it's an
   extension), `ConversationMountsControl`→`useChatPane()` (pane header slot) — so
   the bridge's real consumers reduce to `ProjectDetailPage.reset()` (an action) +
   gallery; see the Round-2 section.)

6. **`sendMessage`/`loadConversation` call the singleton
   `chatExtensionRegistry` (correctness).** The pane store must invoke ITS pane
   runtime, not the global registry. This is a broad in-store edit: every
   `chatExtensionRegistry.X()` in `Chat.store.ts` becomes `runtime.X()`, where
   `runtime` is injected into the pane store by the provider (ITEM-3). The
   store↔runtime handshake is a key wiring detail; if missed, extensions silently
   operate on the wrong pane. Covered by ITEM-2/ITEM-3/ITEM-4.

7. **Right-panel localStorage key (low risk).** Panel snapshots are keyed by
   conversationId already, so per-pane panels persist independently for free —
   *unless* two panes share a conversation (forbidden by DEC-9). PASS.

8. **Performance / virtualization (perf) — SUPERSEDED by the tree-fix.** Message
   virtualization DOES exist on origin/main (`@tanstack/react-virtual` + window
   pagination); it is per-pane in scope (ITEM-2/7, DEC-18 corrected), not "out of
   scope." N panes × virtualized windows is bounded by `MAX_PANES`; the
   measured-height cache is shared/id-keyed. See the Round-2/tree-fix section.

9. **Desktop parity (patterns) — CORRECTED.** Desktop reuses the same chat
   sources, so all edits ride into `src-app/desktop/ui`. The lone desktop consumer
   (`ConversationMountsControl`) is **pane-scoped via `useChatPane()`** (it renders
   inside each pane's header slot), NOT the bridge (DEC-5/DEC-17/ITEM-10 supersede
   the earlier "stays on the bridge" wording). Pop-out also needs a `chat-*` window
   capability grant. Desktop `npm run check` must pass.

## Pattern conformance

- **Multi-instance store:** ITEM-2/3 follow the ONLY production `defineLocalStore`
  precedent (`LlmProviderGroupWidget`) and the store-kit contract exactly
  (`.use({key})`, per-instance `local:<n>` EventBus group, init-on-mount /
  destroy-on-unmount). Conforms.
- **Global store:** ITEM-1 `SplitView` follows `ChatHistory.store.ts`
  (`defineStore` + `immer` + `init:{on,set,actions}`). Conforms.
- **Extension catalog/runtime split (ITEM-4/5):** no existing precedent for
  splitting a registry, BUT the split respects the existing `ChatExtensionRegistry`
  seams — the registration maps are preserved verbatim as the catalog; only the
  store-instance + lifecycle half is lifted into the per-pane runtime. This is a
  refactor-in-place, not a new pattern. Conforms in spirit; the risk is
  mechanical breadth, not architectural novelty.
- **Stream client factory (ITEM-6):** `ChatStreamClient` is already modeled on
  `core/sync/SyncClient.ts`; converting module-singleton state to closure state
  is a standard factory refactor. Conforms.
- **Divider (ITEM-8):** reuses `ResizeHandle` exactly as `ChatRightPanel` does;
  `placement='left'|'right'` is supported for vertical dividers. Conforms.
- **Bridge proxy (ITEM-9):** mirrors `core/stores.ts` `createStoreProxy`
  semantics. Conforms.
- **`Stores.Chat` type alias (patterns).** `modules/chat/types.ts` currently
  types `Stores.Chat` as `ReturnType<typeof useChatStore.getState> &
  ChatExtensionStores`. Converting Chat to a local-store def removes the
  singleton `useChatStore` hook export. The bridge must re-export a
  `useChatStore`-shaped handle (or the type alias must be re-based on the pane
  instance type) so this alias and the dev-gallery fixtures
  (`useChatStore.setState/getState`) still compile. Tracked as CONCERN on
  ITEM-9/ITEM-15.

## Migration collisions

- **No database migration.** This is a frontend-only feature (see the streaming
  finding in PLAN.md). Latest migration on `origin/main` is
  `00000000000132_add_openrouter_provider_type.sql`; this branch adds **none**,
  so there is no numbering collision.
- **No new backend routes, permissions, or sync entities.** The per-pane stream
  model reuses the existing `GET /api/chat/stream` +
  `PUT /api/chat/stream/subscription` + the per-user 12-connection registry
  unchanged. The server-side `active_conversation: Option<Uuid>` per-connection
  scoping is exactly what one-connection-per-pane needs.
- **localStorage key namespace:** ITEM-1 adds a new `SplitView` layout key; it
  must not collide with the existing `ziee-right-panel-tabs-v2` key. New key
  proposed: `ziee-split-view-v1`. No collision.

## OpenAPI regen

- **Not required.** No backend request/response type changes, no new endpoints,
  no schema changes → no `just openapi-regen`, and the golden
  `types_ts_parity` test is unaffected. Consequently neither the `ui` nor the
  `desktop/ui` api-client changes, and the phase-8 backend test chain does not
  apply — only the frontend gates (both workspaces, since desktop mirrors the
  chat UI) run.

## Pop-out (Phase 1) audit

The pop-out tranche is **additive and near-zero-risk to existing chat**: it
introduces a new top-level window/tab that boots an independent SPA instance, so
it touches no existing store/streaming/extension code. Findings:

- **Auth bootstrap (correctness).** A new desktop window MUST re-run the
  `desktop-base` boot (`invoke('auto_login')`) to get its own session; a new web
  tab relies on the shared httpOnly session cookie + the on-401 silent refresh.
  Both paths already exist and fire on SPA init — verify (ITEM-P3) that a
  cold-booted window authenticates without a visible re-login. This is the only
  real correctness risk.
- **SSE connection count (perf/limits).** Each window opens its own
  `chat/stream` connection; N windows + panes stay under the server's per-user
  12-connection cap. No change needed; noted so implementation does not add a
  redundant client cap.
- **Cross-window consistency (state-management).** Two windows are two
  "connections" to the same embedded server; the notify-and-refetch **sync**
  stream already reconciles edits/renames/branch-switches across them. No new
  wiring — reuse, verify in TEST-P4.
- **Shell-native boundary (patterns).** Creating a native window is genuinely
  shell-native, so it correctly uses Tauri's window API, not an Axum route —
  consistent with `[[feedback_no_tauri_ipc]]`'s explicit shell-native exception.
- **No backend change / no migration / no OpenAPI regen** for pop-out either.

## Re-audit pass (6-angle exhaustive; 1 completed, 5 hit the session limit — resets 6:50pm)

A dedicated deep re-audit of `Chat.store.ts` + its couplings surfaced 16 gaps
beyond the original plan; the material ones are now folded in:

- **GAP-1 (extension registry singleton `initialized` + all hooks on the singleton store)** → ITEM-4 (per-runtime `initialized`, hooks on `ctx.chatStore`). Closed.
- **GAP-2 (composer state in FIVE singleton stores: TextStore/FileStore/ModelPicker/AssistantPicker/McpComposer)** → ITEM-5 + DEC-31. Closed pending the consumer grep re-run to confirm no 6th.
- **GAP-3/4 (device-global `desiredConversationId`/`setActiveConversation`/`reset(null)`)** → per-pane stream client (ITEM-6, DEC-2). Closed.
- **GAP-5 (shared `'Chat'` EventBus group → one unmount tears down all panes)** → `ctx.on` per-instance groups (ITEM-2, DEC-32). Closed.
- **GAP-6 (complete/error/ext frames dispatch to registry BEFORE the id guard → cross-pane duplication)** → direct per-pane frame callback replacing the global `chat:token` bus (ITEM-6, DEC-32). Closed.
- **GAP-7 (module-global `lastChatResyncAt` debounce)** → per-instance (ITEM-2). Closed.
- **GAP-8/9 (same conversation in two panes → localStorage panel + message divergence)** → forbidden by DEC-9 (focus the existing pane). Closed.
- **GAP-10/11/12 (extensions/consumers subscribe to the singleton; ~40 reactive consumers; `.$` snapshot reads — already `.$`, not `.__state`, on origin/main)** → `ctx`/`useChatPane` migration (ITEM-5/10); the bridge is out-of-subtree-only. Closed (large surface — ITEM-10 owns it).
- **GAP-13 (`NewChatPage.reset()` targets a pane)** → new-chat-pane adopt-local (ITEM-11). Closed.
- **GAP-14 (auth-stream guard must fire once at boot, device-level)** → ITEM-6 (`streamAuthGuard` at module boot). Closed.
- **GAP-15 (extension-store `createStore()` hardwired to boot-time singleton injection)** → moved to per-pane runtime mount (ITEM-4). Closed.
- **GAP-16 (receiver-pane branch anchor defaults to 'user')** → pre-existing edge, amplified by multi-pane; tracked, low severity.

**Coverage: the 5 sibling audits were re-run and COMPLETED.** Consolidated
findings folded in:

- **Consumers:** composer stores = exactly FIVE (no sixth) — `TextStore` (only one
  nested), `Stores.File`/`ModelPicker`/`AssistantPicker`/`McpComposer` (top-level);
  `Stores.Chat.FileStore` is a dead name; `Stores.McpServer` + id-keyed caches stay
  global. (Correction: `.__state` is ALREADY `.$` on origin/main — 0
  `Stores.Chat.__state`, ~13 files use `.$`; the earlier "24 live `.__state`"
  claim was from the stale checkout.) → DEC-31, ITEM-5, ITEM-10.
- **Extension surface:** full `ChatExtension` hook set (incl. `useSendBlocker`,
  `useConversationMenu`, `conversationHref/BackHref`, `renderConversationCardTrailing`),
  13 `CHAT_SLOTS`, 11 SSE events, 15 extensions. `sseEventHandlers(data,get,set)`
  get/set hardwired to the singleton (`registry.tsx:717`) = #1 change. The
  `initialized` flag no-ops a 2nd pane's `initialize()`. → ITEM-4.
- **MCP interactive flows:** approval reads a global decisions array + calls the
  singleton `sendMessage` → approving in pane B posts to pane A (correctness +
  security-adjacent); `setToolCallProgress` server-keyed cross-bleeds. Elicitation/
  `ask_user` answer by global `elicitation_id` → pane-portable. → ITEM-19, DEC-35.
- **Streaming front+back:** backend is per-conversation/per-message everywhere (no
  per-user active-conversation singleton) — approval/elicitation routing needs NO
  backend change. TWO caveats: connection cap `12` hardcoded (→ configurable,
  DEC-34/ITEM-20) and raw SSE events carry no `conversation_id` (→ one dedicated
  connection per pane, never repointed, ITEM-6). Full enveloped-vs-raw SSE catalog
  recorded (enveloped: started/content/complete/error; raw: titleUpdated,
  mcpToolStart/Complete/Progress, mcpApprovalRequired, mcpElicitationRequired,
  artifactCreated).
- **Nav/header:** `conversation.created` has TWO racing navigate listeners → drop
  event-driven nav (DEC-33). `chatConversationHeaderTrailing` web-empty (only desktop
  `ConversationMountsControl`, which is pane-scoped not bridge — DEC-5 corrected);
  `message_list_header` project chip + `toolbar_status` pills read the singleton
  conversation → pane-scoped (ITEM-10). No drag source exists (ITEM-16). Sidebar
  selected-row must come from `SplitView`, not `location.pathname`.
- **Message-view:** desktop split scroll SAFE (`useNativeScroll` no-ops off-mobile;
  per-pane `DivScrollY`); mobile=tabs sidesteps the un-ref-counted `nativeScroll`.
  The markdown/Shiki/streamdown render spine + `MessageContext` are already
  collision-proof (message-scoped ids) — NO change. `McpToolUseRenderer`'s
  tool-result lookup + `FileAttachmentRenderer.openInRightPanel` must resolve the
  pane (ITEM-19/10).

**What deliberately stays GLOBAL (do NOT pane-scope):** the `ExtensionCatalog`
descriptors + `panelRendererRegistry`, the content-type dispatch + co-ownership
ordering, `Stores.McpServer`, `File.messageFilesCache`/`thumbnailUrls`,
`McpComposer.conversationConfigs`, project `conversationProjectCache`, page stores
(`Memories`/`Projects`), the render spine, and the auth/layout/sidebar/ChatHistory
singletons.

## Round 2 (adversarial feasibility) + tree-fix + re-study — consolidated

**Round 2 caught a fundamental error: the plan was built on a stale checkout
(`/data/pbya/ziee/ziee` @ `786b26890`, 28 commits behind origin/main), missing an
entire message-virtualization + `MessageViewState` subsystem. The worktree is now
reset to origin/main (`90e715c12`).** No architectural blocker survived Round 2 —
the per-pane design is feasible — but these must-fix constraints are folded in:

- **Virtualization/window (tree-fix):** ITEM-2 now owns `hasMoreBefore/After` +
  `loadOlder/Newer/jumpTo/reconcileTail`; ITEM-7 now covers the per-pane
  `useVirtualizer` + top/bottom sentinels + `MessageListHandle` + the 5 window/
  module-singleton collisions (`inPlaceAnchorSignal`, Cmd-F, hashchange deep-link,
  native-scroll composer, `__MSGLIST_METRICS__`). ITEM-21 = `MessageViewState`
  reset-scoping. Keep GLOBAL: `measuredHeightCache`, `estimateMessageHeight`, pure
  helpers. `.__state` is already `.$` here (ITEM-10 premise corrected). run_js adds
  only an SSE event + content-type inside the mcp extension (no new extension/store).
- **Store-kit (Round-2):** `defineLocalStore.use()` doesn't expose `StoreApi` →
  ITEM-22. `.use()` doesn't re-init on conversation change → provider `loadConversation`
  effect (ITEM-2). De-async the store `init` (mount/unmount race). `onFrame` must
  chain through the per-instance `frameApplyTail` (ARCHITECTURE §3 contradiction —
  reconciled). Shared initial nested state → deep-clone per `.use()`. Lost 5s
  destroy-grace → validate StrictMode. List-orchestration methods
  (`conversationHref`/`conversationBackHref`/`renderConversationCardTrailing`/
  `useConversationMenuContributions`) STAY on the global catalog (invoked from
  non-pane surfaces with an explicit `conversation` arg) — not the per-pane runtime.
  `provideUserContent` was missing from ITEM-4's method list — added.
- **Extension module-level hazards (Round-2, same class as `frameApplyTail`):**
  `capturedDraftKey` (text ext), the device-global `globalKeyboardHandler` (keyboard
  ext — needs boot-relocation + resolve-focused-pane-at-action, like `streamAuthGuard`,
  not just querySelector-scoping), and file `unsubConversation`/`unsubEditingMessage`
  handles → all per-pane/relocated (ITEM-5). The bridge type must drop
  `ChatExtensionStores` + `useChatStore` (ITEM-9).
- **Bridge (Round-2):** expose only `$` + actions (a plain-state field silently
  returns non-reactive stale); `useFocusedChatPane()` needs a stable fallback store
  (unconditional hook when `focusedPaneId===null`); `closePane` reassigns
  `focusedPaneId` atomically; registry reads null-tolerant. The bridge's real
  consumers reduce to `ProjectDetailPage.reset()` (an action) + gallery — the
  reactive-read justification is nearly empty.
- **Gallery/Tauri/DnD (Round-2):** gallery harness must migrate `deepStates.tsx`/
  `seededSurfaces.tsx`/`shard5.tsx` off the removed `useChatStore`, add a bespoke
  SplitView-seeding entry (not `?pane=` routing), and regen the tsc-gated
  state-matrix; the single global SSE cassette means the gallery shows at most ONE
  streaming pane unless `mockApi.ts` is extended (ITEM-15). Tauri pop-out needs a
  `chat-*` capability grant (window + outer-position perms) + likely a Rust
  window-builder command for chrome parity (still shell-native) (ITEM-P1..P4/17).
  paneDnd = pointer-based (DEC-36). Native-window TEST-P5 is manual/tauri-driver
  (DEC-39). MessageViewState = DEC-37; store-kit StoreApi = DEC-38.
- **Backend (Round-2 verified):** the connection cap is the ONLY server change;
  all adjacent per-conversation state (host-mounts, sandbox workspace/cgroup/locks,
  summarization per-branch, memory advisory-lock, ephemeral MCP sessions) is already
  N-pane-safe. Sync registry has the same 12-cap (ITEM-20 note).

## Per-item verdicts

- **ITEM-P1** — verdict: PASS — new isolated util; platform-branch on `window.__TAURI__`; no existing caller touched.
- **ITEM-P2** — verdict: PASS — additive affordance reusing existing header-action + row-menu slots; label adapts per platform.
- **ITEM-P3** — verdict: CONCERN — depends on the new window self-authenticating (desktop `auto_login` / web cookie) and deep-linking cleanly; the one thing to verify end-to-end (incl. cross-window sync). No code change to auth — verification-gated.
- **ITEM-P4** — verdict: PASS — dedup by `chat-<id>` window label + title-sync via the existing `conversation.titleUpdated` event; teardown rides normal SPA unload.
- **ITEM-1** — verdict: PASS — new global `defineStore` mirroring `ChatHistory`; adds one localStorage key + URL query mirroring; no existing caller broken.
- **ITEM-2** — verdict: CONCERN — the core conversion; must (a) make `frameApplyTail` per-pane, (b) reroute every `chatExtensionRegistry.X()` to the injected pane runtime, (c) keep `applyStreamFrame`'s conversation-id guards. Broad but mechanical; drift loop must verify streaming + branch + edit/regenerate parity.
- **ITEM-3** — verdict: PASS — standard React context provider mirroring `MessageContext`/`PlusDropdownContext`; owns pane lifecycle; the store↔runtime↔client wiring lives here.
- **ITEM-4** — verdict: CONCERN — no precedent for splitting the registry; risk is mechanical breadth (6 registration maps + ~13 lifecycle/hook methods) not architecture. Catalog stays global; runtime goes per-pane.
- **ITEM-5** — verdict: CONCERN — largest correctness surface: ~13 extensions rewired off the imported `useChatStore` onto a `ctx` handle, incl. the 4 that use imperative `.subscribe` (file/mcp/user-llm-providers/assistant) and the keyboard extension's global `querySelector` (breakage #3). Each extension migrated + verified individually in the drift loop.
- **ITEM-6** — verdict: PASS — module-singleton → factory, mirroring `SyncClient`; auth lifecycle relocated once. N connections are within the server's 12/user cap.
- **ITEM-7** — verdict: PASS — extract pane view verbatim from `ConversationPage`, preserving the `conversation?.id === conversationId` scroll-latch invariants; resolve store via `useChatPane()`.
- **ITEM-8** — verdict: PASS — new container reusing `ResizeHandle`; route wiring; primary pane from URL param, extras from `?pane=` + `SplitView`.
- **ITEM-9** — verdict: CONCERN — the bridge is the migration de-risker but the reactive-across-changing-focus path is the trickiest piece (breakage #5); mitigation = snapshot+action forwarding plus a narrow `useFocusedChatPane()` hook for the ~2 out-of-subtree reactive reads. Must preserve the `Stores.Chat` type alias + gallery fixtures.
- **ITEM-10** — verdict: PASS — mechanical `Stores.Chat` → `useChatPane()` swap in pane-scoped components; each reads/acts on its own pane.
- **ITEM-11** — verdict: PASS — additive affordances (Split button, list "Open in split", per-pane close/focus) gated by `MAX_PANES`; must route new-chat-in-pane through the pane, not a global navigate (breakage #2).
- **ITEM-12** — verdict: PASS — mobile collapses to a single focused pane + tab strip via `SplitView.mode`, reusing existing `useWindowMinSize`/`nativeScroll` gating; the right panel already goes full-screen overlay on mobile.
- **ITEM-13** — verdict: CONCERN — replacing the project extension's `window.location` project-id derivation with a pane-scoped `projectId` is required for correctness of project conversations in non-URL panes; must thread the pane projectId through the `afterCreateConversation` hook context (ties into ITEM-5's ctx change).
- **ITEM-14** — verdict: PASS — named `SPLIT_LIMITS` object; fixed constants with rationale (DEC-15); no server coupling.
- **ITEM-16** — verdict: CONCERN — DnD across sidebar + tab-strip + tile edges is interaction-heavy and a11y-sensitive (drag must have a keyboard/button equivalent — the explicit Split/New-window buttons provide it); must not regress the existing sidebar row DnD (if any). Reuses `SplitView` actions; risk is UX polish + a11y, not architecture.
- **ITEM-17** — verdict: CONCERN — desktop-only tear-off requires detecting a drag leaving the window and creating a `WebviewWindow` at the drop point; the exit-detection is the fiddly part. Fully gated behind `window.__TAURI__`; web silently omits it. Reuses ITEM-P1. Verification-heavy, additive.
- **ITEM-18** — verdict: PASS — reuses the existing mobile full-cover overlay path for the right panel; a mode switch (inline when 1 pane, slide-over when >1). No new panel logic, just a placement branch.
- **ITEM-15** — verdict: CONCERN — new render states (split loaded/empty/streaming, focused/unfocused, mobile tabs) require gallery cells or the `check:state-matrix` gate fails phase 8; the gallery fixtures currently drive `useChatStore` directly (ties into ITEM-9's type-alias concern). The gallery harness also bakes in a "single-active Chat, one reload per combo" isolation assumption that split entries must rework.
- **ITEM-19** — verdict: CONCERN — the deepest content-render surface: `McpComposer` per-pane + two latent-bug fixes (server-keyed progress cross-bleed; approval-routes-to-wrong-pane, correctness + security-adjacent). Bounded (one store + the mcp chat-extension) but must be verified with a real two-pane approval test (TEST-34).
- **ITEM-20** — verdict: CONCERN — the only backend change: make the connection cap configurable + raise it, optionally survive token-refresh without reconnect. Small, non-migration, but it makes the diff back-end-touching → phase-8 runs the backend chain; must not weaken the cap below a safe bound. NOTE the sibling `sync/registry.rs` cap is also 12 and pop-out multiplies both per window.
- **ITEM-21** — verdict: CONCERN — `MessageViewState` is real and was missed (tree-fix), but the fix is light: reset-scoping (re-key by convId), not a full per-pane store — its keys are globally unique. Also folds the run_js chat surface (SSE `runJsApprovalRequired` + `run_js_approval` content-type) into ITEM-19's mcp migration.
- **ITEM-22** — verdict: CONCERN — a genuine store-kit primitive gap: `defineLocalStore.use()` doesn't expose the raw `StoreApi` that `ctx.chatStore` needs. Small store-kit extension (Option A), but it gates the whole extension-migration contract → must land first.
- **ITEM-23** — verdict: CONCERN — the gallery cassette + fixtures are SHARED harness (B3). The multi-pane support (SSE cassette keyed by conversation; fixtures off `useChatStore`) is a real capability gap, not a workaround — lands as a reviewed infra change alongside ITEM-15, not silently.

## v2 redesign audit (ITEM-24..31) — against current merged main

**Note on ITEM-1..23 + P1..P4 above:** these are IMPLEMENTED + verified (v1, 8/8)
and REUSED unchanged; their verdicts stand. The v2 items REVISE five of them —
ITEM-1 (→24/26: drop `?pane=` URL, per-user store), ITEM-8 (→24/29/30:
workspace-driven view), ITEM-11 (→28: picker not new-chat), ITEM-12 (→30: build
mobile tabs), ITEM-16 (→31: build dnd) — captured in the v2 verdicts, not by
re-opening the shipped engine.

### Breakage risk (v2)
- `Stores.SplitView` has exactly **5 consumers** (SplitChatView, module.tsx,
  types.ts, ConversationPage, chatBridge) — **all in-module and all v2 edit
  targets**, so evolving the store breaks no out-of-module caller. LOW.
- The two real risk spots: (a) the **URL↔focus loop** (ITEM-25) — navigating,
  reconciling, and focus-driven URL updates can cycle; and (b) covering **every**
  conversation-click site (ITEM-28: `ConversationCard` × 4 render sites +
  `RecentConversationsWidget`'s own rows). Both are mitigated in-plan (loop guard;
  centralized reroute).
- No `Stores.Chat` engine surface is reopened — the per-pane store/streaming/
  right-panel/composer stay as-shipped.

### Pattern conformance (v2)
- Reuse-first everywhere: `ConversationList` (picker), `chatDrafts.makeDraftKey`
  (per-user persist), kit `Tabs` (tab strip), `SplitDivider` pointer-drag (dnd),
  `SplitView.store.test` reducer tests (reconciliation). **One justified
  deviation:** per-user persistence can't use the store-kit `persist:{name}`
  config (a single global key that can't see the async auth user id) → a custom
  `splitWorkspace.persist.ts` load/save. Recorded as a DEC in phase 4.

### Migration collisions (v2)
- **NONE.** Frontend-only; adds no migration (ceiling 145 untouched).

### OpenAPI regen (v2)
- **NONE.** The workspace layout is client-side localStorage, not a server
  resource — no route/type/schema change, no regen, `types_ts_parity` unaffected.
  (A future server-backed-sync flip would add a settings row + regen — DEC-flagged.)

### Per-item verdicts (v2)
- **ITEM-24** — verdict: PASS — SplitView store already exists; all 5 consumers are in-module v2 edit targets, so evolving it (stable ids, one-conv-per-pane guard, drop `?pane=`) breaks no external caller. Additive.
- **ITEM-25** — verdict: CONCERN — the pure reconciliation reducer is unit-testable in isolation, but the URL↔focus binding risks a navigate↔focus loop; mitigation = `replace` (not push) + skip reconcile when the URL already equals the focused pane's conversation. Verified by the back/forward + deep-link e2e.
- **ITEM-26** — verdict: CONCERN — the current persist is a store-kit GLOBAL key (`ziee-split-view-v1`); per-user namespacing + prune needs a CUSTOM `splitWorkspace.persist.ts` (the built-in `persist` config can't key by the async-loaded `Stores.Auth.user.id`). Hydrate after auth resolves / re-hydrate on auth change; v1→v2 key migration once. Mirrors `chatDrafts` keying.
- **ITEM-27** — verdict: CONCERN — reuse `ConversationList` (it exists + is searchable) but its rows navigate; add an injected `onSelect` (→ `setPaneConversation(thisPane)`) rather than forking the list. Do not rebuild the list.
- **ITEM-28** — verdict: CONCERN — `ConversationCard` renders in 4 sites (ConversationList, registry.tsx, projects extension, ProjectConversationsList) AND `RecentConversationsWidget` has its OWN rows; the reroute must cover ALL of them via one central call to ITEM-25's reducer, and must preserve project-conversation semantics (project binding via ITEM-13). Centralize so no click site is missed.
- **ITEM-29** — verdict: PASS — reuses shipped primitives: pop-out util (ITEM-P1), per-pane sync + self-gate (auto-close on delete), `SPLIT_LIMITS.MAX_PANES` (ITEM-14). Pop-out-moves-out = openConversationWindow + closePane; delete→auto-close = a `sync:conversation` handler → closePane. Additive.
- **ITEM-30** — verdict: PASS — the `mode` field + `useWindowMinSize` gating + kit `Tabs` all exist; building the deferred tab-strip mode is additive to SplitChatView, no new primitive.
- **ITEM-31** — verdict: CONCERN — heaviest v2 item; no dnd library in-repo, so reuse `SplitDivider`'s pointer-drag + shadcn-discovery over a bespoke lib. a11y-sensitive (drag needs a keyboard/click equivalent — the Split button + ⋯-menu + modifier-click already provide it, so this affordance is redundant-optional and can land LAST / be de-scoped if it blocks the gate). Must not regress existing sidebar interactions.
- **ITEM-32** — verdict: CONCERN — the correctness heart of the whole feature (surfaced by human review FB-4): ~16 composer/pane components read `Stores.Chat` directly, and the bridge routes their actions/`.$`/getState/subscribe to `focusedApi()` (the FOCUSED pane) — so Send, model/assistant selection, file attach, and send-blockers are effectively shared across panes (verified: `ChatInput.tsx:29`, chatBridge `:68-73`). The fix (bind each to `useChatPane().store`; make File/ModelPicker/AssistantPicker/McpComposer per-pane not follow-focus) is mechanically broad but low-architecture — it IS the v1 ITEM-5/10/19 work, which DRIFT-1.7 wrongly deemed unnecessary. RISK: it re-touches shipped composer surfaces the v1 8/8 relied on, so the drift loop must re-verify send/edit/regenerate/branch parity per pane, and TEST-31 must assert with NO focus-click. Not a blocker — it's the core deliverable, not an edge.
- **ITEM-33** — verdict: CONCERN — `McpComposer` per-pane; bounded to one store + the mcp chat-extension, but touches the approval/tool-call path the v1 8/8 relied on, so needs a real two-pane approval + config-modal test (TEST-53). Mechanical, not architectural.
- **ITEM-34** — verdict: CONCERN — the registry-runtime per-pane is the highest-leverage fix (one global `cleanup()` + `initialized` flag breaks EVERY pane on any pane's lifecycle event, incl. the close path leaving keyboard dead). Per-pane registry OR paneId-keyed runtime + hook pane-threading; broad but the catalog/runtime split was already scoped as v1 ITEM-4 (deferred by DRIFT-1.1). Verify pane-close does not disarm survivors (TEST-54).
- **ITEM-35** — verdict: CONCERN — the streaming frame-routing fix is the single strongest correctness item (same-conversation splits garble live text). Tag frames by originating client/pane + own-frame filter is a small, localized change to `ChatStreamClient`+`applyStreamFrame`; the risk is getting the same-conversation dedup right without dropping legitimate cross-device frames. Must have the two-branch streaming test (TEST-55).
- **ITEM-36** — verdict: CONCERN — right-panel rebind + pane-scoped persistence; real data-loss (dropped exclusion reason) makes it HIGH-priority, but it's a mechanical `Stores.Chat`→`useChatPane().store` sweep over the panel renderers/actions + a persistence key change. Verify no regression to the shipped single-pane right panel (TEST-56).
- **ITEM-37** — verdict: CONCERN — header/chrome + new-chat sentinel keys; several small independent fixes (summarization read-model, find/deep-link/title scoping, key namespacing). Low-architecture; the new-chat-key namespacing must not break the per-conversation re-key that already works for existing conversations (TEST-57).
- **ITEM-38** — verdict: CONCERN — message-render actions + tool-result renderers rebind; subsumes v1 ITEM-10/ITEM-21. The wrong-conversation export + regenerate-on-wrong-pane are HIGH, but each is a `useChatPane().store` rebind + capture-before-await; the `resetViewState` no-op is a one-line ordering fix. Broad file count, low risk each (TEST-58).
- **ITEM-39** — verdict: CONCERN — module singletons / global-DOM; the keyboard `document.querySelector` first-match (Ctrl+Enter always leftmost) is a genuine per-pane-scoping change to a shared listener (B3-adjacent: the keyboard extension is shared, but the fix is per-pane scoping, not a workaround); markdown anchors + project-from-URL are localized. Verify the keyboard extension still works single-pane (TEST-59).

## Iteration round 2 — coverage-gap DELTA audit

Merged onto current origin/main (@304f4a011): no migration collision (branch adds
no migrations); openapi regenerated for BOTH workspaces; `npm run check` green in
ui + desktop/ui. This round adds tests + a pure module + a doc — no new UI surface,
no backend, no permission, no new render state.

- **ITEM-40** — verdict: PASS — mirrors `mcp/stores/approvalRouting.ts` (pure enum-free extraction so `node:test` can load it); `File.store` delegates with byte-identical behaviour, so it breaks no existing caller — the existing `14-split-chat` e2e suite is the delegation regression guard. No migration, no openapi.
- **ITEM-41** — verdict: PASS — a new e2e spec mirroring `composer-isolation.spec.ts` + `right-panel-per-pane.spec.ts`'s `attachFileRobust`; touches no product code, only `tests/e2e/`. Asserts existing per-pane behaviour without a focus-click (per FB-4).
- **ITEM-42** — verdict: PASS — a durable Markdown doc under `tests/e2e/14-split-chat/` (survives the `.lifecycle` merge-strip, DEC-56) + a structural unit test; no product-code risk.

## Iteration round 3 — explicit open-conversation choice (FB-8)

- **ITEM-43** — verdict: PASS — localized + additive. (a) `dialog.choose` is a new
  method on the existing imperative kit host — the `confirm`/alert paths are
  untouched (byte-identical), so no existing dialog caller is affected. (b)
  `needsOpenChoice` is a new PURE export in `reconcile.ts` — reads only its inputs,
  independently unit-testable, changes no existing reducer path. (c) the
  `useOpenConversation` change is a guarded pre-step before the existing reducer
  call; the non-ambiguous paths (single-pane, already-open, explicit intent) fall
  through unchanged. No migration, no openapi, no new permission, no cross-module
  coupling (the two feature-module extensions that read `SplitView` are untouched).
  A new imperative-dialog render state may need a `check:state-matrix` gallery cell
  or allowlist reason — handled at phase 8.

## Iteration round 4 — single-pane pop-out is desktop-only (FB-9 / DEC-60)

- **ITEM-44** — verdict: PASS — a one-line render gate in `OpenInNewWindowAction`
  driven by a new PURE `popoutActionVisible(inPane, isDesktop)` (its own module +
  unit test, mirrors `needsOpenChoice`). Platform gate via the existing runtime
  `'__TAURI__' in window` check ([[feedback_platform_gating]]) — desktop keeps the
  single-pane button, web hides it; split panes unaffected on both. No new
  coupling, no migration, no openapi. Desktop shares ui's component (no desktop
  copy), so the runtime check covers both bundles. The existing `popout-new-tab`
  e2e (TEST-P3/P4) pops out from single-pane WEB, which this hides — so that spec
  is re-pointed at a SPLIT pane (where the button remains) + a gating case added.

## Iteration round 5 — split-awareness of main's new modules (Stage 2)

Merged onto current origin/main@6b56d0d14; no migration collision (branch adds none;
DEC-62 keeps the tool-call fix frontend-only → no new migration/openapi). Survey-backed:
message-stream cluster is already split-safe; these are per-pane correctness fixes of
existing composer/right-panel surfaces (no new surface, no permission).

- **ITEM-45** — verdict: CONCERN — voice touches an imperative MediaRecorder singleton
  (module-level `let`s) + focused-bridge reads; the fix is real per-pane rework (bind to
  owning pane, exclusive-recorder guard, pane-scoped focus). Reference patterns exist
  (file send-blocker `useChatPaneOrNull`+`composerPaneKey`; keyboard `focusedPaneRoot`).
  Product fork DEC-61 (exclusivity A1/A2) must be confirmed before implementing.
- **ITEM-46** — verdict: CONCERN — `KnowledgeBaseComposer` is a global singleton; making
  selection per-conversation mirrors `McpComposer` exactly. Risk: the extension's
  onConversationLoad path currently drives the one global store per pane (race); the fix
  must not regress single-pane KB grounding. Covered by TEST-69/70.
- **ITEM-47** — verdict: PASS — smallest: `McpComposer` config is already conversation-
  keyed; only the `McpStatusRow` visible-selection resolve needs to read the owning pane's
  conversation. Localized. Covered by TEST-71.
- **ITEM-48** — verdict: PASS — a frontend `message_id`→conversation filter on the existing
  seed+scroll loops in `ConversationPage`; no backend change (DEC-62). Extract the filter
  pure for TEST-72. Verify single-pane scroll-to-approval is unchanged.
- **ITEM-49** — verdict: CONCERN — re-keying `PdfHighlight` by (paneKey,fileId) touches the
  store + the pdf viewer body read + cleanup; mirror `File.store` composerPaneKey. Must not
  regress single-pane citation highlight. Covered by TEST-73/74.
- **ITEM-51** — verdict: PASS — mirrors the existing per-conversation keying already in
  BOTH composers; the pending key simply gains a `:<paneId>` suffix (null pane → bare key,
  so single-pane is byte-identical, verified against `PENDING_KB_KEY`/`PENDING_CONVERSATION_KEY`
  callers). No migration, no OpenAPI regen (frontend-only, no request/response shape change).
  Breakage risk contained: the new `paneId` params are OPTIONAL (existing callers still
  compile), the pure key helpers live in the already-node-testable `kbSelectionKey.ts` /
  `approvalRouting.ts`, and the read sites reuse the proven `useChatPaneOrNull()?.paneId`
  pattern (same portal-context path TEST-69/71 exercise). MCP's added `currentPaneId` is only
  consulted in the pending branch of `resolveConfigKey`, so committed-conversation + project
  scopes are unchanged. Covered by TEST-76/77/78.
- **ITEM-52** — verdict: PASS — layout is route-controlled (`RouterComponent` renders a
  layout-less route bare: `route.layout || null`), so a new `/chat-window/:conversationId`
  route with no `layout` renders `ConversationPage` without the app shell — no new render
  path, no ConversationPage change (same `:conversationId` param). Web unaffected (the web
  pop-out still opens `/chat/:id`); only the desktop `.desktop.ts` override retargets the
  WebviewWindow url. Proven by RUNNING the render (TEST-79 e2e asserts the DOM: shell absent
  + composer/title present) — not a code-read. Covered by TEST-79 + TEST-75.
- **ITEM-53** — verdict: PASS — a new web/desktop seam `focusPopoutWindowIfOpen` (web: `false`
  → open inline unchanged; desktop: `WebviewWindow.getByLabel` → focus) injected as a single
  guard at the top of the sole open-conversation entry point (`useOpenConversationInWorkspace`).
  Web behaviour byte-identical (always false). Shared `popoutWindowLabel` removes the duplicate
  label literal (was inline in `openConversationWindow.desktop.ts`). Desktop control flow RUN by
  TEST-80 (Tauri mocked, the established TEST-75 seam pattern — the crux is window-focus control
  flow, NOT render). Covered by TEST-80.
- **ITEM-54** — verdict: PASS — pure `planPopoutSnapBack`/`handlePopoutClosed` decide the
  snap-back (never duplicate, never past MAX_PANES) and are RUN by TEST-81/82; the desktop
  cross-window wiring (`popoutSnapBack.desktop.ts`: pop-out emits `popout-closed` on close, main
  window listens → `handlePopoutClosed`) is a web/desktop seam whose emit/listen control flow is
  RUN by TEST-83 (Tauri boundary mocked). Web no-op (seam base). Mounted per-role by the route
  (pop-out route's `PopoutConversationPage` registers the emitter; `AppLayout`, which the
  layout-less pop-out route does not render, registers the main listener). The only thing NOT
  runnable in this Linux env is the Tauri cross-OS-window event DELIVERY itself — a platform
  guarantee, not owned logic — noted for desktop-host verification. Covered by TEST-81/82/83.
- **ITEM-55** — verdict: PASS — pure render-gate: the back arrow is conditionally rendered on
  `!isSplit && !isPopoutWindow` (both reactive/route-derived facts). No handler change; a
  single `!showBackButton && ...` wrap. `isSplit` reads the existing `SplitView.panes` (the
  same source SplitChatView keys off), `isPopoutWindow` the route. Zero effect on single-pane
  (both false → shown). RUN by TEST-85 (real DOM: present single-pane, absent split + pop-out).
- **ITEM-56** — verdict: PASS — the split button becomes `{!isPopoutWindow && <Tooltip>…}` in
  ConversationPage, and `popoutActionVisible` gains a third `isPopoutWindow` param (default
  false → all existing callers unchanged) that short-circuits to false; `OpenInNewWindowAction`
  passes it. A shared `useIsPopoutWindow()` (one `useLocation().startsWith('/chat-window/')`)
  is the single source, removing the duplicated route literal. All hooks called before the
  component's early returns (Rules of Hooks). Web/main-window behaviour byte-identical
  (isPopoutWindow false everywhere except the pop-out route). RUN by TEST-65b (pure — false in
  a pop-out window across pane/platform) + TEST-85 (real DOM — split + pop-out buttons absent in
  the pop-out window, find + composer still present).
- **ITEM-50** — verdict: PASS — pure structural migration of the one raw desktop whole-file
  shadow (`desktop/ui/.../openConversationWindow.ts`) to the live2 co-located
  `ui/src/.../openConversationWindow.desktop.ts` mechanism; mirrors the existing
  `api-client/getBaseURL.desktop.ts` precedent (ui tsconfig excludes `*.desktop.ts`, so the
  Tauri import is not typechecked in the web workspace; `localOverridePlugin` resolves it in
  the desktop bundle). No behavior change — the WebviewWindow contract is byte-identical; the
  TEST-P5 desktop unit re-points its import to the `@/…desktop` alias (Drawer.test.ts
  convention). Gated by `gen-override-registry.mjs --check` (0 web-only) inside `npm run
  check` in BOTH workspaces. Covered by TEST-75.
- **ITEM-57** — verdict: PASS — the drop handlers self-gate on `!pane` + a `conversation`
  drag-kind, so they never fire in a split pane (each pane's ConversationPage has `pane != null`)
  and never cross-fire with the composer's FILE drop (`dragKind` disambiguation, the same guard
  ITEM-31 relies on). `openPane` is the exact store API `onSplit` already uses (appends,
  dedups a same-conversation, caps at MAX_PANES); left/right just differ in seed order. Center
  reuses the canonical `useOpenConversationInWorkspace` single-pane open. The testid is
  conditional (`pane ? undefined : …`) so the testid-registry stays unique. No new migration/
  OpenAPI. Pattern-conform: mirrors the existing pane-header drop handler shape in the SAME file.
  RUN by TEST-88/89 (pure) + TEST-90 (e2e — aimed clientX per third).
- **ITEM-58** — verdict: PASS — reuses the ITEM-P1 `openConversationWindow` seam (web
  `window.open` / desktop `WebviewWindow`) and the ITEM-29 MOVE (`closePane`) exactly as the ⤢
  button (`OpenInNewWindowAction`) does — no new window machinery. The desktop-only + strict
  gate lives in the PURE `planTearOff` (open only when `isOutside && isDesktop`), so web is a
  no-op (byte-identical to today). `onDragEnd` is additive on the existing drag sources (no
  change to `onDragStart`/drag payloads). `isDesktop` uses the same `'__TAURI__' in window`
  probe as `OpenInNewWindowAction`. No migration/OpenAPI. RUN by TEST-91/92 (pure geometry +
  decision + exec glue with spies) + TEST-93 (e2e web desktop-only gate).

## Plan-coverage correction (ITEM-16 / ITEM-17 were paper-covered)

Auditing the plan against the codebase for this round surfaced that the ORIGINAL ITEM-16
(in-tile edge drop-zones Split-left/Replace/Split-right) was reduced to only `reorderPanes`
(TEST-27) with the edge-drop deferred, and ITEM-17 (desktop tear-off) was mapped to TEST-28
(`drag-to-split.spec.ts`) which does NOT exercise tear-off at all — so both shipped as
"covered on paper" but never genuinely built. ITEM-57 genuinely implements the single-pane
edge-drop half of ITEM-16 (the split view's edge cases already ship via ITEM-31's
header=replace + seam=new-pane); ITEM-58 genuinely implements ITEM-17. This is recorded as
FB-14 and DRIFT-10 rather than silently absorbed.

## Independent completeness audit — the prior "9/9" was a PAPER-9/9 (recorded, not absorbed)

An independent auditor found the prior 9/9 was a paper-9/9: real per-pane bugs shipping
unfixed + HOLLOW tests (a passing test line that never exercised its claimed behavior).
This is exactly the "prove by RUNNING, not reading; every test MUST exercise its item"
discipline (rule B7). All 11 items fixed this round with REAL covering tests across two
ACTIVE panes. Per-item verdicts:

- **ITEM-59** — verdict: PASS — global `{open}` → `openConversationId`-keyed; both composer
  slots read the pane's own conversation (`useChatPaneOrNull`, sibling McpMenuItem pattern);
  detail sub-drawer gated to the open pane (no N copies). RUN by TEST-94 (count=1).
- **ITEM-60** — verdict: PASS — the window keydown now returns early unless
  `pane.paneId === SplitView.$.focusedPaneId` (single-pane `!pane` always focused). RUN by TEST-95.
- **ITEM-61** — verdict: PASS — TitleEditor binds `useChatPaneOrNull()?.store`; save + read
  are the pane's. RUN by TEST-96 (server-verified the RIGHT conversation renamed).
- **ITEM-62** — verdict: PASS — hooks resolve `ownerChatState(ownerPaneId)` (Chat.store passes
  `get().paneId`) + `PaneDraftKeys` per-pane capture; mirrors the e2e-tested File-extension
  ownerPaneId pattern. RUN by TEST-97 (unit, clobber-safety).
- **ITEM-63** — verdict: PASS — real two-pane approval e2e proves the resume routes to the
  owning pane's conversation (captured send URL), tool call mocked at the SSE boundary. TEST-98.
- **ITEM-64** — verdict: PASS — card was already pane-correct (ITEM-38 `useChatPaneOrNull`); the
  phantom TEST-58 claim is corrected + the real export-per-pane e2e added. TEST-99.
- **ITEM-65** — verdict: PASS — real per-pane view-state isolation e2e (canvas edit toggle). The
  same-file-id-in-two-panes literal is prevented by the dedup guard; view-state is local `useState`
  per FilePanel instance (documented). TEST-100.
- **ITEM-66** — verdict: PASS — 3-pane close-during-record e2e (avoids the collapse-remount
  confound). TEST-101.
- **ITEM-67** — verdict: PASS — ConversationFindBar binds the pane store; search-scope e2e. TEST-102.
- **ITEM-68** — verdict: PASS — EditingMessageBanner + CanvasSelectionPopover adopt the pane store;
  the editing banner per-pane assertion folded into TEST-58 (TEST-103).
- **ITEM-69** — verdict: PASS — two-simultaneous-streams bidirectional e2e (both panes active),
  replacing the idle-empty-pane control. TEST-104.

### Unproven-in-CI (marked explicitly, NOT claimed as CI-passing proof)

- Cross-window snap-back (TEST-81/82/83/84) + desktop tear-off native-window positive (TEST-93):
  pure-logic + spies + the WEB gate only. The Tauri emit/listen round-trip + a real
  `WebviewWindow` open are DESKTOP-HOST-ONLY (externally verified), never exercised in this
  Linux CI. Recorded in FB-15/FB-16; do not read these as CI-passing proof of the native path.

### Architecture-vs-plan divergence — see DEC-30 amendment (impl-wins drift)

DEC-30's "clean cut" (remove `useChatStore`, split registry into catalog + per-pane runtime)
did NOT fully land: `useChatStore` still exists (~7 files) and `Stores.Chat` is a bridge/shim.
It works (pane-rebound SSE + ownerPaneId hooks) but the plan overstated completion — amended
honestly at DEC-30 as impl-wins drift, not left claiming a removal that didn't happen.

## ITEM-70 verdict (per-pane edge-directional drop in existing splits)

- **ITEM-70** — verdict: PASS — generalizes the shipped single-pane edge-drop (ITEM-57)
  to split panes, fulfilling the ORIGINAL DEC-25 edge-drop-zone intent that ITEM-31
  under-delivered (DRIFT-12.1). Pure `planSplitPaneDrop` mirrors `planSinglePaneDrop`;
  store `openPane({beforePaneId})` is symmetric with `afterPaneId` (unit-tested); the
  column handler dispatches single-pane vs split by `pane` and dedups via the store's
  one-conversation-per-workspace guard; the cap falls back to replace. The header handler
  is narrowed to pane-reorder only and a conversation drag falls through to the column
  (event bubbling; the column is the ancestor that preventDefaults dragover). The
  Rules-of-Hooks regression (reactive `Stores.SplitView.panes` in the overlay `.map()`)
  was caught by the human RUNNING the live app and fixed to a `.$` snapshot — verified by
  the drag-to-split e2e that renders the overlay mid-drag (DRIFT-12.2). RUN by TEST-105/107
  (unit) + TEST-106 (e2e). npm run check green both workspaces.

## ITEM-71 verdict (split header matches the app header)

- **ITEM-71** — verdict: PASS — the fix REUSES/shares the sibling's logic rather than
  re-deriving it: `useHeaderLeftInset()` (core web 48/12 + `.desktop` 118/48/12 macOS)
  is the single source of truth now consumed by `HeaderBarContainer` core + desktop AND
  the split leftmost pane header, so the inset can't drift. Pane header height set to
  50px to match. The `z-10`→`z-[5]` focus-ring change keeps the ring above sibling panes
  (z-auto) while dropping below the fixed `z-10` toggle, making the collapse button
  clickable. The new `.desktop.ts` hook is a registered override seam (OVERRIDE_MANIFEST
  regenerated, 15 .desktop files). RUN by TEST-108 (real click + computed-style asserts).
  npm run check green both workspaces.
- **ITEM-72** — verdict: PASS — completes the URL↔workspace contract the ITEM-25 comment
  already asserted but only half-built. The new effect is loop-SAFE: it only fires on
  `[focusedConvId, panes.length]` changes (NOT `conversationId`), and its equality guard
  makes it a strict no-op when the URL already matches, so it cannot ping-pong with the
  URL→workspace reconcile (which, in turn, no-ops once focus matches). No new migration,
  no OpenAPI/type change (pure client navigation). `focusedConvId` is derived from the
  already-reactive `panes`/`focusedPaneId` — no store proxy read inside a loop/conditional.
  Single-pane path (`panes.length < 2`) is an early-return no-op → zero behavior change to
  the non-split view. Verified live before the covering spec; RUN by TEST-109.
