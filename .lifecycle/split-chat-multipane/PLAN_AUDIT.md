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
