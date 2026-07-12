# DECISIONS — split-chat-multipane

Every human/product input the implementation needs, resolved up front so
implementation runs nonstop. Each resolution is a recommendation grounded in an
existing convention or the codebase; the ones most worth the user's confirmation
before implementation are flagged **[confirm]** and summarized in the halt
message.

### DEC-20: How is multitasking delivered — pop-out, in-window split, or both?
**Resolution:** **Both, sequenced.** Phase 1 = pop-out (open conversation in a
new window/tab), shipped first as a low-risk, all-platform, independently-PR-able
tranche. Phase 2 = the in-window per-pane split. The two affordances coexist.
**Basis:** user — selected "Both — pop-out now + split later".

### DEC-21: Pop-out mechanism per platform?
**Resolution:** Desktop = a native Tauri 2.8 `WebviewWindow` (from
`@tauri-apps/api/webviewWindow`, guarded by `window.__TAURI__`); web =
`window.open('/chat/:id', '_blank')`. Opening a native window is shell-native, so
it uses the Tauri window API directly, NOT an Axum route.
**Basis:** codebase — Tauri 2.8 supports multi-window; `[[feedback_no_tauri_ipc]]`
explicitly exempts shell-native operations from the Axum-over-IPC rule.

### DEC-22: How does a freshly-opened window authenticate (no token passing)?
**Resolution:** The new window boots the SPA and self-authenticates — desktop via
the existing `desktop-base` → `invoke('auto_login')` (permanent local session);
web via the shared httpOnly session cookie + the on-401 silent refresh. No token
is passed between windows.
**Basis:** codebase — `Auth.store` (cookie/silent-refresh) + `desktop-base`
`auto_login` fallback already bootstrap a session on SPA init.

### DEC-23: Pop-out window dedup?
**Resolution:** One window per conversation, keyed by the label `chat-<id>`;
reopening a conversation that already has a window focuses it rather than
duplicating (mirrors DEC-9's no-duplicate-conversation rule for panes).
**Basis:** convention — consistency with the split's no-duplicate-conversation
invariant; avoids two live streams for one conversation on one device beyond the
cap headroom.

### DEC-24: What is the split-view layout / chrome? (design-tournament outcome)
**Resolution:** **Tab-strip workspace** — a browser-like pane-tab strip across the
top (each open conversation is a tab: live dot, close ✕, "＋", and a strip-right
toolbar) over **framed workspace tiles** on a muted background, with an
**active-tile ring** (no dimming of the other pane). Won a 3-variant on-token
vision tournament (28 vs Tiled 27 vs Primary+companion 23), synthesized with
grafts (see DEC-25/26/27).
**Basis:** user — chose "run a design-variant tournament"; the vision judge
scored the tab-strip direction highest for brief fit (it unifies pane / window /
tab / mobile under one control surface).

### DEC-25: The interaction model — how are panes/windows created? (the core UX)
**Resolution:** **One unified drag gesture, decided by drop target:** drag a
conversation (from the sidebar) or a pane-tab → (a) drop on a pane's **edge
drop-zone** = *split* (Split-left / Replace / Split-right), (b) drag a tab within
the strip = *reorder* (drop indicator), (c) drop **outside the app window** on
desktop = *tear off into a new OS window*. Web can't spawn OS windows on drop, so
there it supports drag-to-split + the explicit "New window/tab" button only.
Explicit buttons ("Split ▐▌", "New window ↗") are the discoverable, non-drag
fallback on every platform.
**Basis:** user — "user can drag the conversation on the sidebar or the chat page
outside the window to open it in a new window… think about UX too"; mirrors the
Chrome-tab tear-off mental model.

### DEC-26: How does the right panel work in split? (design tension #1)
**Resolution:** In split/tab-workspace mode the right panel renders as a
**per-pane slide-over inside its own pane** (reusing the existing mobile
full-cover overlay behavior), NEVER a third inline column — so two panes each
opening a panel can't starve each other of width.
**Basis:** design — a 4-column `[chatA|rpA|chatB|rpB]` layout over-crams a normal
screen (flagged in PLAN_AUDIT breakage #7 / this UX pass).

### DEC-27: Pop-out vs split affordance distinctness? (design tension)
**Resolution:** Distinct icons + copy: **split** = a columns glyph (accent-toned
"Split ▐▌"), **pop-out** = an external-arrow "New window ↗". Per-pane headers
carry a ⤢ pop-out + ✕ close; the strip toolbar carries both split + new-window.
**Basis:** user requirement that the split affordance be visually distinct from
the pop-out affordance.

### DEC-28: Focused/active-pane indicator?
**Resolution:** The active pane's tile gets a 2px `--ring` outline + slight
elevation; the other pane is NOT dimmed (you must read both streams). The active
tab in the strip carries the same accent underline.
**Basis:** design — clear focus without harming legibility of the unfocused pane
(concretizes DEC-16's focus model into the winning chrome).

### DEC-29: Mobile fallback shape? (concretizes DEC-11)
**Resolution:** On small screens the **pane-tab strip becomes the mobile tab
nav** — one visible pane at a time, switch by tab; no side-by-side columns and no
drag-to-split. This falls out of the tab-strip layout for free.
**Basis:** design — the winning layout's tab strip already IS the mobile control;
DEC-11 chose tabs, this fixes the exact shape.

### DEC-30: How far does the extension API migration go in one step?
**Resolution:** **Clean cut.** Change `initialize()` (and the store-reaching
hooks) to take `PaneExtensionCtx`, migrate ALL chat extensions off the global
`useChatStore` in one pass, and **remove the `useChatStore` singleton export** so
no extension can couple to a global chat store. No focused-pane `useChatStore`
shim / incremental path.
**Basis:** user — chose "ctx everywhere (clean cut) — no residual singleton
coupling".

### DEC-31: Which stores hold composer state, and do they all go per-pane? (audit GAP-2)
**Resolution:** Exactly FIVE composer/conversation-scoped stores go per-pane
(consumer grep confirmed — no sixth): `TextStore` (the only one nested under
`Stores.Chat`), and the TOP-LEVEL singletons `Stores.File` (the `Stores.Chat.FileStore`
name is dead), `Stores.ModelPicker`, `Stores.AssistantPicker`, `Stores.McpComposer`.
`Stores.McpServer` (deployment-wide registry) and the id-keyed caches
(`File.messageFilesCache`, `McpComposer.conversationConfigs`, project caches) stay
GLOBAL. Each of the five is a per-pane instance owned by the pane runtime.
**Basis:** codebase (audit) — the composer is not one store; leaving any of these
singleton means two panes share one draft/model/assistant/MCP selection.

### DEC-32: How are stream frames routed to a pane, and how are listeners scoped? (audit GAP-5/GAP-6)
**Resolution:** Each pane's `ChatStreamClient` drives its own store via a **direct
callback** (`onFrame → thisPane.store.applyStreamFrame`), NOT the global
`chat:token` EventBus — so a pane never receives another pane's frames and the
pre-guard extension dispatch can't duplicate across panes. The store's remaining
EventBus listeners (`sync:conversation`, `sync:reconnect`, `chat:stream-reconnect`)
use the store-kit `ctx.on` seam so each instance gets its own `local:<n>` group
(the shared `'Chat'` group + `removeGroupListeners('Chat')` would tear down all
panes' listeners on one unmount).
**Basis:** codebase (audit) — removes cross-pane frame leakage + listener-group
collision; both are latent bugs a naive per-pane conversion would ship.

### DEC-33: How is post-create navigation handled with N panes? (nav audit GAP-E1)
**Resolution:** Drop event-driven navigation. Today `conversation.created` is a
global, origin-less EventBus event with TWO page-level listeners (`NewChatPage`
AND `ProjectDetailPage`) that each `navigate()` — with a pane + the project page
(or two panes) mounted, one create fires COMPETING navigations for the single
window. The initiating pane/page instead adopts the returned conversation id
directly (`createConversation` already returns it → `SplitView.setPaneConversation`
or a scoped navigate for the primary pane); the `conversation.created` event stays
ONLY for the `ChatHistory` list store to prepend the row.
**Basis:** codebase (audit) — the un-tagged global event + multiple navigate
listeners is a concrete multi-pane routing hazard.

### DEC-34: The connection cap — fixed const, config, or admin settings row? (streaming audit GAP-2)
**Resolution:** Make `PER_USER_MAX_CONNECTIONS` (chat-stream `registry.rs:26`,
today a hardcoded `12`) a **deployment config value** with a raised default (e.g.
24), NOT a bare const and NOT a per-deployment admin-UI settings row. Rationale:
it is a low-level transport resource cap analogous to the sibling
`GLOBAL_MAX_CONNECTIONS` const, not a product feature — a config knob (like the
existing `jwt.*` seed values) fits; promotable to a settings row later if needed.
Also reduce reconnect churn (let the SSE stream survive an access-token refresh)
so the cap isn't hit by token-refresh storms. This is the mandatory
configurable-settings DEC for the one operational tunable this feature adds.
**Basis:** convention — the feature-lifecycle configurable-settings rule (explicit
choice + rationale + structured, not a magic number); the true peer is a transport
const, not an admin feature.

### DEC-35: How are MCP tool-calls / approvals / config made pane-safe? (extension + message-view audits)
**Resolution:** Move `McpComposer`'s per-conversation fields into the pane runtime
(`toolCalls`, `approvalDecisions`, `elicitationRequests`, `selectedServers`,
`currentConversationId`, `currentProjectId`, `configModalVisible`; keep
`conversationConfigs` global). Fix two latent bugs exposed by multi-pane: (a)
`setToolCallProgress` is keyed by SERVER → re-key by `(server, message_id)` (the
progress event lacks `tool_use_id`; else two panes on one MCP server cross-bleed
progress); (b) the approval action reads the global
decisions array + calls the singleton `sendMessage` → route through the pane's
decisions + the pane's `sendMessage` (else approving in pane B posts to pane A's
conversation — a correctness + security-adjacent bug). Elicitation/`ask_user`
answer by global `elicitation_id` so they stay pane-portable.
**Basis:** codebase (audit) — these are outright data collisions / mis-routing a
naive per-pane conversion would ship.

### DEC-36: Is paneDnd pointer-based or HTML5 drag-and-drop? (Round-2 test/tauri audit)
**Resolution:** **Pointer-based** for the in-app drag (sidebar→pane, tab reorder,
tile edge drop-zones), mirroring the existing `ResizeHandle` (`onMouseDown`,
Playwright-testable). Reserve `dragend.screenX/Y` (HTML5) ONLY for the desktop
tear-off-past-window-bounds detection (ITEM-17), which is mouse-only + manual-test
anyway. Rationale: Playwright can't synthesize HTML5 `dataTransfer`, so an HTML5
in-app DnD would make TEST-28 unrunnable; there's no existing HTML5-DnD/dnd-kit in
the repo to reuse.
**Basis:** codebase — `ResizeHandle` pointer precedent + Playwright DnD limits.

### DEC-37: How is `MessageViewState` made pane-safe? (tree-fix)
**Resolution:** Reset-scoping, not a full per-pane store. Its maps are keyed by
globally-unique message-id / file-URI so the data can't collide; the only bug is
the GLOBAL `resetViewState()` fired on one pane's conversation switch/teardown
wiping the other pane's state. Re-key both maps by conversationId and make reset
clear only the outgoing conversation's sub-map (thread convId into the
`CollapsibleBlock`/`InlineFilePreview` selectors). Keep the store global (its
key-space is safe); only its reset is scoped.
**Basis:** codebase — the store's own comment says reset "is not required for
isolation" (keys are unique); a full instancing would be over-engineering.

### DEC-38: How does `ctx.chatStore` get a raw `StoreApi` from a per-pane store? (Round-2 store-kit gap)
**Resolution:** Extend `defineLocalStore` to expose the underlying `StoreApi`
(subscribe/getState/setState) as a return field (**Option A, preferred**). NOT the
raw-`createStore`-bypass (Option B) — `makeBuilder`/`applyMiddleware` are
module-private, so bypassing `.use()` would drop the subscribeWithSelector + immer
+ persist + `__init__`/`__destroy__` stack. `defineLocalStore.use()` returns only
the read-proxy today, but the **7** `useChatStore.subscribe` extension sites (file
×2, assistant ×2, user-llm ×2, mcp ×1) + the `sseEventHandlers(data,get,set)`
rebinding require the raw api.
**Basis:** codebase — the extension-migration contract (ITEM-5) structurally needs
`StoreApi`, which the current primitive doesn't surface.

### DEC-39: Is the desktop native-window test automatable? (Round-2 test audit)
**Resolution:** No — TEST-P5 (native second `WebviewWindow` on desktop) is
**manual / tauri-driver-only**. Desktop e2e is Playwright-chromium with a MOCKED
Tauri (`installTauriMock` in every spec), so `new WebviewWindow` never spawns a
real OS window. This is a legitimate platform/tooling gate (not a "skip to go
green"): the web pop-out (new-tab) + the `openConversationWindow` unit test cover
the logic; the native window is verified manually or via a `tauri-driver` smoke.
**Basis:** codebase (`playwright.config.ts` + `tauri-mock.ts`) + the no-skip rule's
genuine-platform-incompatibility exception.

### DEC-1: How does the per-conversation chat store go from singleton to per-pane?
**Resolution:** Convert `Chat.store.ts` to a `defineLocalStore` def
(`ChatPaneStore`) instantiated once per pane via `.use({ conversationId, paneId })`,
resolved through a `ChatPaneProvider` React context (`useChatPane()`). NOT a
paneId-keyed map inside a still-singleton store (that would rewrite every action
to be pane-keyed) and NOT iframes-per-pane (breaks the meta-framework, ×N
SSE/layout, shared-auth hazards — considered and rejected).
**Basis:** codebase — `defineLocalStore` is the store-kit's sanctioned
multi-instance primitive; `LlmProviderGroupWidget.store.ts` is the exact
one-instance-per-row precedent.

### DEC-2: How does a stream land in the correct pane (streaming routing)?
**Resolution:** One SSE connection **per pane** — a per-pane `ChatStreamClient`
instance scoped (via the existing subscription PUT) to that pane's conversation;
frames route into the owning pane's store. NOT a single connection with a
subscription *set* (that would need the backend to envelope the currently
**unenveloped** raw extension events with their conversation id, a change to
hardened server code) — deferred as a future optimization.
**Basis:** codebase — the server `chat/stream` registry already supports N
connections/user each scoped to one conversation, with per-conversation
generation slots; one-connection-per-pane reuses that scoping exactly.
**Caveat (streaming re-audit):** raw SSE events carry no `conversation_id`
(attributed purely by the connection's subscription), so each pane keeps a
DEDICATED connection that is torn down + re-opened on a conversation switch,
never repointed (ITEM-6). And the per-user connection cap must be raised/made
configurable (DEC-34). So it is *almost* frontend-only — one small backend
change, not zero.

### DEC-3: What stays global vs per-pane? [confirm]
**Resolution:** **Global** — auth/session, app layout + sidebar, `ChatHistory`
(conversation list), the extension *registration catalog* (slots/handlers/
renderers), and the new `SplitView` layout store. **Per-pane** — conversation,
messages, streaming state + stream connection, scroll, right-panel, composer
input/text, model selection, file attachments, MCP tool-approvals, and the
extension *runtime* (store instances + lifecycle).
**Basis:** user goal (own input / own streaming / own scroll / own right-panel)
+ codebase (which state is conversation-scoped vs deployment/user-scoped).

### DEC-4: How are chat extensions made pane-independent?
**Resolution:** Split `ChatExtensionRegistry` into a global `ExtensionCatalog`
(descriptors, populated once at module load) + a per-pane `PaneExtensionRuntime`
(store instances + lifecycle/hooks bound to the pane's chat store). Extensions'
`initialize(ctx)` receives the pane's chat-store + extension-store via `ctx`
instead of importing the global `useChatStore`.
**Basis:** codebase — the registry already separates static registration maps
from per-conversation runtime; extension stores already come from
`defineExtensionStore` (per-call instances).

### DEC-5: Big-bang consumer migration, or a compatibility bridge?
**Resolution:** A **narrow** `Stores.Chat` bridge remains, but ONLY for the
handful of **non-extension, non-subtree** consumers — the desktop
`ConversationMountsControl` and the dev-gallery fixtures — via snapshot + action
forwarding to the focused pane plus a `useFocusedChatPane()` hook for the ~2
reactive reads. Pane-subtree components move to `useChatPane()`; chat extensions
do the clean `ctx` cut (DEC-30), so they do NOT ride the bridge. The bridge is a
thin focused-pane accessor, not an incremental-migration crutch. **Re-audit
correction:** the desktop `ConversationMountsControl` is NOT a bridge consumer —
it renders inside each pane's header (via the per-pane `chatConversationHeaderTrailing`
slot) so it resolves `useChatPane()`. The bridge's only real consumers are
`ProjectDetailPage.reset()` (a non-pane page driving the active pane) + the
dev-gallery fixtures.
**Basis:** user (DEC-30 clean cut) + convention — smallest surface that keeps the
few genuinely-global consumers working without a singleton chat store.

### DEC-6: How many panes in v1, and the hard cap? [confirm]
**Resolution:** Ship **2 panes** in v1; build the store/layout as an ordered list
so N ≥ 2 is a data change, not a rewrite; hard cap `MAX_PANES = 3`.
**Basis:** user goal ("2+") + codebase (the server's 12-connection-per-user cap
is the real resource bound; 3 panes × a couple devices stays well under it).

### DEC-7: Split direction? [confirm]
**Resolution:** **Vertical side-by-side columns** on desktop (matches "side by
side"); horizontal stacking is deferred (not in v1).
**Basis:** user goal wording + codebase (the chat column is already a flex row
with the right panel; a second column drops in naturally).

### DEC-8: Divider control?
**Resolution:** Reuse `ResizeHandle` (`placement='left'|'right'`) between pane
columns, exactly as `ChatRightPanel` uses it; per-divider width stored in
`SplitView`.
**Basis:** codebase — `ResizeHandle` already supports vertical dividers with
keyboard a11y, min/max clamps, and an `onEnd` commit.

### DEC-9: Can two panes show the SAME conversation? [confirm]
**Resolution:** **No** — opening a conversation already open in another pane
focuses that pane instead of duplicating it. This keeps the shared-EventBus
frame guards, the conversationId-keyed right-panel localStorage, and the
per-conversation generation slot unambiguous.
**Basis:** codebase — `applyStreamFrame` and the right-panel snapshots key on
conversationId; duplicates would double-apply frames and collide on storage.

### DEC-10: How does a user open a second pane? [confirm]
**Resolution:** v1 = (a) a "Split" button in the conversation header (opens a
second pane as a new chat) + (b) an "Open in split pane" item in the
conversation-list / `RecentConversationsWidget` row menu. Drag-a-conversation-
into-the-split is deferred to a follow-up.
**Basis:** convention — mirrors existing header actions + row context menus;
lowest-friction affordances without new drag infrastructure.

### DEC-11: Mobile / small-screen behavior? [confirm]
**Resolution:** Below the `useWindowMinSize` breakpoint, collapse the split to a
single **focused** pane + a tab strip to switch panes (`SplitView.mode='tabs'`);
no simultaneous columns. Open-in-split adds a tab rather than a column.
**Basis:** codebase — the chat already switches to native window-scroll + a
full-screen right-panel overlay on mobile; columns do not fit narrow screens.

### DEC-12: How is the split represented in the URL?
**Resolution:** Primary pane stays in the path (`/chat/:conversationId`);
additional panes ride a `?pane=<id>` query param (general form `?pane=a&pane=b`),
so the split is deep-linkable and survives reload. The `SplitView` store mirrors
localStorage + URL.
**Basis:** convention — mirrors the right-panel's per-conversation persistence;
keeps existing single-pane routes valid.

### DEC-13: pendingProjectId / project binding under split?
**Resolution:** `pendingProjectId` does not exist (the CLAUDE.md note is stale).
Replace the project extension's `window.location.pathname` project-id derivation
with a **pane-scoped** `projectId` carried on the pane, so a project conversation
in a non-URL pane binds to the correct project.
**Basis:** codebase — project binding is currently URL-path-derived at
create-time, which is ambiguous once >1 conversation is on screen.

### DEC-14: Keep the per-conversation `conversationStateCache` per pane?
**Resolution:** Keep it, per-pane (a pane can still switch its own conversation
via the sidebar), so back/forth within a pane stays instant. Do not attempt a
cross-pane shared cache.
**Basis:** codebase — lowest behavior-change risk; the cache is already an
in-store optimization scoped by conversationId.

### DEC-15: Is `MAX_PANES` (and pane-size limits) a fixed constant or an admin-configurable settings row? [confirm]
**Resolution:** **Fixed frontend constants**, structured as a named
`SPLIT_LIMITS` object (`MAX_PANES`, `MIN_PANE_WIDTH`, `MAX_PANE_WIDTH`,
`DEFAULT_DIRECTION`) — NOT an admin settings row. Rationale: the only real
operational resource this feature consumes is SSE connections, which the server
ALREADY bounds + owns via the per-user 12-connection cap in the chat-stream
registry; max-panes is a pure client-side ergonomics limit. Structuring it as a
`Limits`-style object (not inline magic numbers) leaves it promotable to
configurable later without a rewrite, satisfying the configurable-settings rule's
fixed-constant exception.
**Basis:** convention — the feature-lifecycle configurable-settings rule (fixed
constant permitted with rationale + `Limits` struct); the true resource bound is
server-side and already enforced.

### DEC-16: Focus model — what is "the focused pane"?
**Resolution:** The last pane the user interacted with (clicked into, typed in,
or sent from) is `focusedPaneId`; it shows a focus ring and is the target of
global affordances (keyboard shortcuts, export). A newly opened pane becomes
focused.
**Basis:** convention — standard multi-pane focus semantics; matches how global
keyboard shortcuts must resolve a single target (audit breakage #3).

### DEC-17: Desktop parity?
**Resolution:** Desktop reuses the same chat sources, so the feature ships to
desktop automatically; the lone desktop consumer (`ConversationMountsControl`)
is **pane-scoped via `useChatPane()`** (it renders inside each pane's header via
the `chatConversationHeaderTrailing` slot), NOT the bridge — this supersedes the
earlier "stays on the bridge" wording and matches DEC-5/ITEM-10. Verify with
`npm run check` in BOTH `ui` and `desktop/ui`. No desktop api-client regen (no
backend type change), but pop-out DOES need a `chat-*` window capability grant +
possibly a Rust window-builder (Round-2 Tauri finding, ITEM-P1..P4/17).
**Basis:** codebase — desktop embeds the same UI; `[[project_desktop_embeds_server]]`
and the OpenAPI-both-binaries rule (moot here — no backend change).

### DEC-18: Message virtualization for N heavy panes?
**Resolution:** CORRECTED (tree-fix) — virtualization is NOT out of scope; it
already EXISTS on origin/main (`@tanstack/react-virtual` + window pagination +
`MessageViewState`), and making it per-pane IS in scope (ITEM-2/7/21). It is not
a perf-mitigation choice — it's an existing subsystem the per-pane conversion
must carry. The original "no virtualization" premise came from studying a stale
checkout 28 commits behind origin/main; the worktree is now on origin/main.
**Basis:** codebase (tree-fix re-study) — `MessageList.tsx` `useVirtualizer`, the
store's `hasMoreBefore/After` + `loadOlder/Newer/jumpTo/reconcileTail`, and the
`MessageViewState` store are all present on the real base.

### DEC-19: Does the split apply to the projects `/projects/:projectId/chat/:conversationId` route too?
**Resolution:** Yes — both `/chat/:conversationId` and the projects chat route
render `SplitChatView` (the projects route already re-imports the conversation
page). Panes from different projects are allowed (each carries its own pane
projectId per DEC-13).
**Basis:** codebase — the projects chat route is literally the conversation page
re-imported; unifying on `SplitChatView` keeps them consistent.

## v2 redesign decisions (workspace interaction layer)

### DEC-40: What IS a split — the foundational mental model?
**Resolution:** A **Workspace** — a persistent set of 1..N open conversations, one
per pane (IDE editor-groups). You add/remove conversations; it survives navigation
+ reload; the URL is a *view into* it, not its source of truth.
**Basis:** user — chosen explicitly over "current + peek" and "tabbed panes".

### DEC-41: Plain sidebar-click on a conversation NOT already in a pane, while a split is open?
**Resolution:** **Replace the focused pane** (IDE "open in the active editor
group"). Modifier/middle-click opens a new pane instead.
**Basis:** user — chosen over "always new pane" and "exit split to single view".

### DEC-42: How is the pane layout stored — fixed client state, or an admin-configurable/server row?
**Resolution:** **localStorage, per-user** (client-side UI state). NOT server-backed
and NOT an admin settings row for v1. Server-backed cross-device sync is a noted
future upgrade (would then become a `session`-style settings row + a sync entity +
regen — see DEC-52).
**Basis:** user — chose localStorage-per-user over server-sync and session-only.

### DEC-43: Which "open an existing conversation beside" affordances ship in v1?
**Resolution:** **All four** — empty-pane picker, ⋯-menu "Open in split pane",
drag-and-drop, and modifier/middle-click. (Drag-drop is the lowest-priority of the
four — see DEC-50.)
**Basis:** user — multi-selected all four.

### DEC-44: `MAX_PANES` — fixed constant, or admin-configurable settings row?
**Resolution:** **Fixed frontend constant** in `SPLIT_LIMITS` (`MAX_PANES = 3`, `4`
at ≥1600px viewport). NOT an admin settings row: it is a per-user UI *layout* limit
bounded by screen real estate + readability, analogous to the existing
`SPLIT_LIMITS`/`MIN_PANE_WIDTH` constants — not a deployment operational tunable.
Over-cap → toast + a replace-focused offer.
**Basis:** convention — the configurable-settings rule targets *deployment* tunables
(resource/retention/quota caps); a client UI layout cap is a fixed const with
rationale.

### DEC-45: Pop-out (⤢) on a pane while in a split — keep it in the split, or move it out?
**Resolution:** **Move it out** — pop-out opens the conversation in its own window
AND removes that pane from the workspace, so there are never two live copies of one
conversation competing (double SSE stream / double send target).
**Basis:** user — confirmed the recommended option.

### DEC-46: May the same conversation be open in two panes at once?
**Resolution:** **No** (v1) — adding a conversation already in a pane focuses that
pane instead. Two live panes on one conversation would double-stream + compete on
send. Revisit later for a deliberate "compare two branches of one conversation".
**Basis:** convention — matches the one-live-connection-per-conversation engine
invariant; avoids the double-send/stream hazard.

### DEC-47: "New Chat" (sidebar) while a split is open?
**Resolution:** **Adds a new-chat pane** to the workspace (consistent with the
workspace = "you add things" model), not replace-focused and not exit-split.
**Basis:** convention — coherent with DEC-40's workspace model.

### DEC-48: URL model + history behavior for the workspace?
**Resolution:** URL = the **focused pane's conversation** (`/chat/<focused>`); focus
changes update it with **`replace` (not push)** so focusing panes doesn't spam
browser history; a **loop-guard** skips reconciliation when the URL already equals
the focused pane's conversation (prevents the navigate↔focus cycle flagged in
PLAN_AUDIT ITEM-25).
**Basis:** codebase — react-router `navigate(..., {replace:true})`; the guard is the
standard fix for URL-mirrors-state loops.

### DEC-49: Mobile / narrow-viewport behavior?
**Resolution:** **Build the tab strip** — below the `useWindowMinSize` breakpoint,
`mode:'tabs'` renders ONE visible pane + a tab per open conversation (no columns);
"open beside" becomes "add a tab". (The v1-deferred DRIFT-1.11 piece; now in scope.)
**Basis:** user (workspace model implies it) + convention (`useWindowMinSize`
gating mirrors `ConversationPage`/`ChatRightPanel`).

### DEC-50: Drag-and-drop (ITEM-31) sequencing — required, or optional-last?
**Resolution:** **Optional-last.** The menu item + modifier/middle-click + empty-pane
picker already deliver "open existing beside" with a keyboard/click path; drag-drop
is a redundant power affordance. It lands LAST and may be de-scoped to a follow-up
if it threatens the gate (no dnd lib in-repo; a11y-sensitive). Its e2e (TEST-28)
still ships if built.
**Basis:** codebase — a11y + no dnd primitive; the affordance is redundant with
three already-locked paths.

### DEC-51: Persistence mechanism — store-kit `persist` config, or custom load/save?
**Resolution:** **Custom `splitWorkspace.persist.ts`** load/save. The store-kit
`persist:{name}` writes a single GLOBAL key and cannot namespace by the
async-loaded `Stores.Auth.user.id`; the custom module keys per-user (mirroring
`chatDrafts.makeDraftKey`), hydrates AFTER auth resolves (re-hydrates on auth
change), prunes deleted/inaccessible + empty panes on hydrate, and runs the
one-time v1→v2 key migration.
**Basis:** codebase — the store-kit persist limitation is real (PLAN_AUDIT ITEM-26).

### DEC-52: Configurable-settings-rule check — any deployment operational tunable introduced?
**Resolution:** **None.** The redesign is frontend-only; its two "tunables"
(`MAX_PANES` and the localStorage layout) are **client UI state**, fixed with
rationale (DEC-44/DEC-42), not deployment operational tunables — so no settings
table, migration, `<feature>::settings` permission, sync entity, or admin card is
owed. (A future server-backed-sync flip is the only path that would introduce one.)
**Basis:** convention — the mandatory configurable-settings DEC, answered:
fixed/client, not admin-configurable, with rationale.

### DEC-53: How much of the composer is truly per-pane — full isolation, or bridge/follow-focus? (FB-4)
**Resolution:** **Full per-pane isolation of all FIVE composer stores** —
`TextStore` (done), `File` (attachments), `ModelPicker` (model), `AssistantPicker`
(assistant), `McpComposer` (MCP/approval). Each pane-scoped composer component
binds to `useChatPane().store`, so its ACTIONS + snapshot reads target its own
pane; the stores are per-pane-instanced or strictly pane-keyed (not the global
`NEW_CHAT` key that makes two new-chat panes share). This **supersedes
DRIFT-1.2/1.3/1.4** (which left File/Assistant/MCP follow-focus and Model
new-chat-key-shared) and **DRIFT-1.7** (bridge ≠ isolation for actions). The
`Stores.Chat` bridge is retained ONLY for genuinely out-of-subtree consumers
(DEC-5), not for anything inside a pane subtree.
**Basis:** user — FB-4 ("two chat inputs … still share state"); the partial v1
isolation is a confirmed defect, so full per-pane is required, not follow-focus.

### DEC-54: Can v2 "reuse the v1 engine unchanged", or must the whole chat surface be migrated? (FB-5 + ISOLATION_AUDIT)
**Resolution:** **NO — the "reuse unchanged" assumption is REJECTED.** The
converged multi-agent audit (`ISOLATION_AUDIT.md`, 79 findings → 34 issues in 10
groups) proved the entire chat surface shares state across panes via three root
causes: (a) process-global singleton stores (`File`/`McpComposer`/`Summarization`/
`MessageViewState`/right-panel persistence/new-chat sentinel keys), (b)
`chatBridge` resolving ACTIONS + `.$` snapshots to the FOCUSED pane, (c)
module/DOM/EventBus side-channels (`capturedDraftKey`, `inPlaceAnchorSignal`, the
one keyboard `document` listener, `document.querySelector` first-match,
`window.location`, the single global `EventBus`). ITEM-32..39 migrate all of it.
Only the pane INFRASTRUCTURE (`ChatPaneStore`/`ChatPaneProvider`/per-pane
`ChatStreamClient`/`SplitChatView`/pop-out/divider) is reused. DRIFT-1.1's
"skip the catalog/runtime registry split" and DRIFT-1.7's "the bridge obviates
migrating ~40 consumers" were both **wrong** — the bridge covers reactive reads
only. "Reuse unchanged" would ship a demonstrably corrupting UI (garbled
same-conversation streaming, wrong-pane exports/sends, dropped right-panel input,
Ctrl+Enter always hitting the leftmost pane).
**Basis:** user (FB-5 "you need to check everything … basically everything") +
codebase (the converged audit's concrete per-file findings).

## Iteration round 2 — coverage-gap DELTA decisions

### DEC-55: Where does the per-pane file-ownership logic get unit-tested — in `File.store`, or an extracted pure module?
**Resolution:** Extract the owner-filter + backup-snapshot + restore-MERGE into a pure, enum-free `composerOwnership.ts`; `File.store` delegates to it. Unit-test the pure module with `node:test`.
**Basis:** convention — `File.store.ts` imports `@/api-client`/`@/core/stores` (which pull the `Permissions` enum), and `node:test` strip-only mode cannot load enums; this is the identical constraint that made `approvalRouting.ts` a pure extraction out of `McpComposer.store`. Mirror that precedent.

### DEC-56: Where does the durable `COVERAGE_GAPS.md` live so it survives to main?
**Resolution:** `src-app/ui/tests/e2e/14-split-chat/COVERAGE_GAPS.md` (next to the split-chat specs).
**Basis:** codebase — `.lifecycle/` is `git rm`-stripped at merge (skill "Merge hygiene"), so a doc placed there would not reach main; placing it beside the specs keeps it durable and discoverable by future test authors. Its covering unit test (TEST-62) reads it by a path relative to the test file.

### DEC-57: Are the 5 candidate bugs the sweep found (text error-restore not pane-threaded, Cmd-F window-global, TitleEditor focused-bridge binding, beforeSendMessage approval read, same-file view-state) fixed THIS round?
**Resolution:** DEFERRED — recorded in `COVERAGE_GAPS.md` as tracked, prioritized follow-ups; not fixed this round.
**Basis:** user — the human scoped this iteration round to exactly the 3 delta items (composerOwnership extraction + per-pane unit suite, the file-attach/send-blocker/assistant e2e, and the committed COVERAGE_GAPS.md). The candidate bugs are surfaced + documented (not silently cut), to be verified + fixed in a subsequent round.

### DEC-58: When does the 3-way open-conversation prompt (single / replace / new) fire?
**Resolution:** ONLY the ambiguous case — a plain (`auto`) click, while a split is
open (≥2 panes), on a conversation that is NOT already in a pane. Single-pane
clicks navigate instantly (no prompt); an already-open conversation focuses its
pane (no prompt, one-conversation-per-workspace); Cmd/middle-click + the `⋯`
"Open in split pane" menu keep their explicit `newPane` intent (no prompt).
**Basis:** user — chosen directly via the AskUserQuestion ("Only ambiguous case")
in the review thread that raised FB-8; keeps routine navigation snappy while
resolving the one genuinely-ambiguous route.

### DEC-59: How is the 3-way choice implemented so it stays modular (FB-8 mandate)?
**Resolution:** A REUSABLE imperative `dialog.choose({options[]})` primitive added
to the shared kit dialog host (`components/ui/kit/dialog-host.tsx`, additive to
`confirm`/alert) that resolves the chosen option key (or null), PLUS a PURE
`needsOpenChoice(...)` predicate in `split/reconcile.ts`. The chat hook only wires
them — NOT a chat-specific dialog component.
**Basis:** convention + user (FB-8) — mirrors the existing imperative-dialog seam
(`dialog.confirm`) and the pure-reducer pattern (`openConversationInWorkspace`); a
general kit primitive is reusable app-wide and adds no cross-module coupling.

### DEC-60: Should the single-pane "Open in new tab/window" pop-out button exist on the web?
**Resolution:** DESKTOP-ONLY in single-pane. Inside a split pane the pop-out shows
on both platforms (it is the "move this pane out" action). In single-pane it shows
ONLY on the desktop app; on the WEB the single-pane button is hidden.
**Basis:** user — chosen via AskUserQuestion in the review thread ("why would a
user want to open in a new tab for a single pane?"). Rationale: on desktop a
native OS window is the only way to get a second top-level view, so the button is
the sole affordance; on the web the browser already provides "open in new tab"
(Cmd/middle-click, duplicate tab), so an in-app single-pane button is redundant
chrome. Implemented as a pure `popoutActionVisible(inPane, isDesktop)` gate.

### DEC-61: Voice recording with two panes — exclusive (one at a time) or simultaneous?
**Resolution:** EXCLUSIVE recording, per-pane STATE — **A1 (disable others while
recording)**, confirmed by the human at plan approval. The mic is single hardware
(one `getUserMedia` stream), so true simultaneous recording is impossible. Each
pane owns its own VoiceStore instance (caption/status/transcript destination), but
the imperative recorder is a single guarded resource owned by the pane that started
it; the transcript lands in THAT pane's TextStore. While a recording is active, the
mic button in OTHER panes is **disabled/busy** (A1) — NOT silently superseding the
active recording (A2, rejected).
**Basis:** user — chosen via AskUserQuestion at Stage-2 plan approval ("Disable
others while recording"); single mic hardware makes exclusivity a physical constraint.

### DEC-65: Are the surveyed out-of-scope surfaces approved as descoped for round 5?
**Resolution:** Yes, approved. The human approved the Stage-2 plan with the descope
list as-is (AskUserQuestion "All 5 this round" → the 5 ITEMs are in scope and the
proposed descopes stand). Out of scope (no in-pane surface exists to make pane-aware):
(a) web/lit/bio search composer affordances (none exist — backend auto-attach only;
the tool-result cards + lit screening panel are already pane-safe); (b) voice /
web-search / literature / citations / scheduled-tasks / model-download-upload ADMIN +
settings pages (standalone routes, deployment-wide singletons, never in a pane);
(c) `mcpExtension.beforeSendMessage` reading the focused pane's approvals (intentional
ITEM-33 "sending = focused"). [approved: user@2026-07-11 · Stage-2 plan approval]
**Basis:** user — Stage-2 plan approval.

### DEC-62: How is a tool-call's originating conversation resolved for the scroll-to-approval filter?
**Resolution:** FRONTEND, via the tool-call's `message_id` → the pane whose messages
contain that message_id. No backend change (no new `conversation_id` column on
`McpToolCall`, no migration, no openapi regen).
**Basis:** codebase — `message_id` is already tracked on `McpToolCall` for progress
correlation, and each pane's own `messages` map holds its message ids; a frontend
resolve is lighter than a backend schema change.

### DEC-63: How is the per-pane KB grounding selection keyed?
**Resolution:** Conversation-keyed Maps in `KnowledgeBaseComposer.store`, resolved
via the owning pane's conversation — mirroring `McpComposer.store`'s
`conversationConfigs` / `resolveConfigKey` (which the branch already relies on for
MCP). The status row + picker read/write the OWNING pane's conversation's selection.
**Basis:** convention — `McpComposer` is the established per-conversation composer-
selection pattern in this exact area.

### DEC-64: How is the per-pane PDF citation highlight keyed?
**Resolution:** Key the `PdfHighlight` target (+ the file find-query) by
`(paneKey, fileId)` instead of `fileId` alone, mirroring `File.store`'s
`composerPaneKey` ownership; the pdf viewer body reads its own pane's key and
cleanup scopes to that key.
**Basis:** convention — `File.store`'s per-pane `composerPaneKey` ownership is the
established pattern for per-pane file state on this branch.

### DEC-66: How is the desktop `openConversationWindow.ts` whole-file shadow handled vs live2's incoming override gate?
**Resolution:** ACTIVE ITEM-50 (live2 landed @3d58b011a — no longer deferred). live2's
desktop-override mechanism (element `<Seam>`/`useOverride` or co-located
`*.desktop.tsx`) + its enforcing `gen-override-registry.mjs --check` gate (in
`npm run check`, both workspaces) + `desktop/ui/OVERRIDE_EXCEPTIONS.md` are now on main.
My branch merged main at 6b56d0d14 (BEFORE this), so the gate isn't in my tree yet, but
the next main-sync (needed anyway before merge-gate) picks it up and WILL flag my raw
shadow `desktop/ui/.../popout/openConversationWindow.ts`. Migrate it to a co-located
`src-app/ui/src/modules/chat/core/popout/openConversationWindow.desktop.ts` (whole-file
`.desktop` override — the right granularity for Tauri-`WebviewWindow`-specific code, NOT
an element seam), else an approved `OVERRIDE_EXCEPTIONS.md` shadow-exception with the
Tauri-API structural reason. Covered by TEST-75. Sequencing (per the human): done at the
NEXT main-sync, in the SAME checkpoint, AFTER the current per-pane items (45/46/47/49).
**Basis:** user — coordination FYI (live2 merged @3d58b011a; migrate at next main-sync).
