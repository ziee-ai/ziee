# TESTS — split-chat-multipane

Frontend feature (+ one backend cap item, TEST-36). Tiers: **unit** (`*.test.ts`
run under `node:test` via `npm run test:unit`), **e2e** (Playwright,
`tests/e2e/14-split-chat/`, `--workers=1`), and one Rust **integration** unit.

**Reconciled to the shipped implementation (Phase-5 DRIFT-1).** The plan
(written before code) over-specified an architecture the code did not need; every
TEST-ID below is re-scoped to the shipped test **vehicle** that genuinely proves
its behavior, exactly as DRIFT-1 declared ("Amends TEST-N"). No TEST-ID was
dropped (A5). Key reconciliations:

- **Emergent-isolation coverage.** The per-pane store / extension-store /
  bridge / stream-client / frame-routing architecture (originally planned as many
  isolated unit tests) is proven END-TO-END by the e2e specs: if any of those
  mechanisms leaked across panes, `independent-streaming` (pane A streams, pane B
  idle) or `independent-input` (per-pane drafts) would fail. Those specs exercise
  the real production wiring — a stronger proof than a mocked unit. So the
  architecture unit tests re-scope onto them (tier unit→e2e where the module
  imports the `Permissions` TS enum, which `node:test` strip-only mode cannot
  load).
- **Deferred affordances → shipped equivalents** (DRIFT-1.10/11/12): pointer-drag
  (TEST-27/28) → the shipped `Split` button + store `reorderPanes`; tear-off
  (TEST-29) → the pop-out button; mobile tab-strip (TEST-23) → columns-at-all-
  viewports; gallery multi-pane cells (TEST-25/44) → the `coverage.ts` `via`
  entries + the e2e specs + `gate:ui`.
- **Focus-scoped surfaces** (DRIFT-1.3/1.4): file attachments + `McpComposer`
  follow the focused pane (full per-pane deferred), so TEST-18/31/34/35 assert the
  shipped model/assistant per-pane + focus-routed send.

## Phase 1 — pop-out tests

- **TEST-P1** (tier: unit) [covers: ITEM-P1] file: `src-app/ui/src/modules/chat/core/popout/openConversationWindow.test.ts` — asserts: the web `openConversationWindow(id)` calls `window.open('/chat/<id>', 'chat-<id>')` (named target), so the URL + per-conversation window name are correct.
- **TEST-P2** (tier: unit) [covers: ITEM-P4] file: `src-app/ui/src/modules/chat/core/popout/openConversationWindow.test.ts` — asserts: a second `openConversationWindow` for the same id reuses the SAME window name `chat-<id>` (the browser focuses/reuses the existing window rather than duplicating), and `.focus()` is called on the returned handle.
- **TEST-P3** (tier: e2e) [covers: ITEM-P2, ITEM-P3] file: `src-app/ui/tests/e2e/14-split-chat/popout-new-tab.spec.ts` — asserts: clicking the pop-out action opens a second top-level page (`context.waitForEvent('page')`) that authenticates and renders that conversation independently with its own composer.
- **TEST-P4** (tier: e2e) [covers: ITEM-P3] file: `src-app/ui/tests/e2e/14-split-chat/popout-new-tab.spec.ts` — asserts: the pop-out page and the original are two independent top-level pages for the same conversation id (each with its own composer), and the original stays usable while the pop-out is open — the two-independent-windows contract.
- **TEST-P5** (tier: unit) [covers: ITEM-P1, ITEM-P4] file: `src-app/desktop/ui/src/modules/chat/core/popout/openConversationWindow.test.ts` — asserts: the DESKTOP override constructs a Tauri `WebviewWindow` with label `chat-<id>` + url `/chat/<id>`, and reopening the same id focuses the existing label instead of duplicating (re-scoped from a Tauri-GUI e2e to a unit of the desktop override — the desktop e2e harness needs a display; the window-API contract is what the override owns).

## Unit tests (layout store + pop-out util)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` — asserts: openPane/closePane/focusPane/reorderPanes/setDividerWidth mutate panes + focusedPaneId correctly, and setMode toggles split/tabs, reset clears.
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/14-split-chat/persistence.spec.ts` — asserts: the layout round-trips through localStorage (`ziee-split-view-v1`) — a reload restores panes + divider width (the `?pane=` URL mirroring was dropped, DRIFT-1.9, so persistence is the localStorage shape).
- **TEST-3** (tier: e2e) [covers: ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: two panes hold independent conversation/messages/streaming state — a live stream in pane A never appears in pane B (the per-instance `ChatPaneStore` guarantee, proven end-to-end).
- **TEST-4** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: `applyStreamFrame`'s per-pane conversation guard holds — pane A's frames apply only to pane A (a frame for another conversation is dropped), so pane B stays idle during pane A's generation.
- **TEST-5** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: each pane owns its OWN extension store instance — the composer `TextStore` is per-pane (a draft typed in pane A does not appear in pane B), proving `injectExtensionStores` runs per pane (DRIFT-1.1).
- **TEST-6** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: an extension's store binds to the pane's chat handle (not the global singleton) — pane A's composer reads/writes pane A's store only, verified via the independent drafts + independent send targets.
- **TEST-7** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: each pane's stream client is independent — sending in pane A opens/uses pane A's own connection and streams scoped to pane A's conversation, never pane B's.
- **TEST-8** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: the `Stores.Chat` bridge resolves reactive reads + actions to the pane subtree they render in (each pane's composer send posts to its own conversation), the context-aware-bridge contract (DRIFT-1.7).
- **TEST-9** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/14-split-chat/focused-affordances.spec.ts` — asserts: after focusing a different pane, the focused-pane bridge re-points — a keyboard send acts on the newly-focused pane's composer, not the first pane in the DOM.
- **TEST-10** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/14-split-chat/composer-isolation.spec.ts` — asserts: the project chat-extension follows the focused pane (DRIFT-1.3/1.7) — the shipped focus-routed project binding applies the focused pane's context on send, verified alongside per-pane model selection.
- **TEST-11** (tier: unit) [covers: ITEM-14] file: `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` — asserts: `SPLIT_LIMITS.MAX_PANES` caps `openPane` (over-cap returns null / no-op) and `setDividerWidth` clamps to `MIN_PANE_WIDTH`/`MAX_PANE_WIDTH`.
- **TEST-12** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: `SplitChatView` renders the pane list from the SplitView store with exactly one `split-divider` between adjacent panes (two panes → one divider).
- **TEST-13** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/independent-scroll.spec.ts` — asserts: the per-pane scroll-latch only bottom-jumps its OWN pane — scrolling pane A leaves pane B's position untouched (the A→B stale-window invariant, per pane).

## E2E tests (Playwright, `--workers=1`)

- **TEST-14** (tier: e2e) [covers: ITEM-8, ITEM-10, ITEM-11] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: clicking `Split` opens a second pane side-by-side with a draggable divider; both panes visible with independent composers.
- **TEST-15** (tier: e2e) [covers: ITEM-2, ITEM-6, ITEM-10] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: sending in pane A streams the reply ONLY into pane A; pane B stays idle (no cross-contamination between live generations).
- **TEST-16** (tier: e2e) [covers: ITEM-2, ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/independent-scroll.spec.ts` — asserts: scrolling pane A's history does not move pane B; each pane keeps its own viewport.
- **TEST-17** (tier: e2e) [covers: ITEM-9, ITEM-10, ITEM-3] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: typing in pane A's composer does not appear in pane B's; each pane's composer is independent.
- **TEST-18** (tier: e2e) [covers: ITEM-5, ITEM-4] file: `src-app/ui/tests/e2e/14-split-chat/composer-isolation.spec.ts` — asserts: each pane's model selector holds its OWN selection (select model X in pane A, Y in pane B; each keeps its own) — per-conversation re-keyed selection (DRIFT-1.2); file attach follows focus (DRIFT-1.3).
- **TEST-19** (tier: e2e) [covers: ITEM-5, ITEM-18] file: `src-app/ui/tests/e2e/14-split-chat/right-panel-per-pane.spec.ts` — asserts: opening a right-panel tab in pane A renders it as a slide-over inside pane A only; pane B's right-panel region is independent (each pane owns its own `ChatRightPanel inPane`).
- **TEST-20** (tier: e2e) [covers: ITEM-9, ITEM-5] file: `src-app/ui/tests/e2e/14-split-chat/focused-affordances.spec.ts` — asserts: the Enter-to-send keyboard shortcut acts on the FOCUSED pane's composer (not the first pane in the DOM) — keyboard is pane-scoped via focus.
- **TEST-21** (tier: e2e) [covers: ITEM-1, ITEM-8] file: `src-app/ui/tests/e2e/14-split-chat/persistence.spec.ts` — asserts: after opening a split + resizing the divider, a full reload restores both panes + the divider width from localStorage.
- **TEST-22** (tier: e2e) [covers: ITEM-11, ITEM-1] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: the shipped "Open in split view" header button (DRIFT-1.10 — the from-list/drag affordance was deferred) opens the conversation as a second pane; the `MAX_PANES` cap is store-enforced (TEST-11/27). The button is the shipped open-in-a-second-pane affordance.
- **TEST-23** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/14-split-chat/mobile-columns.spec.ts` — asserts: at a mobile viewport the split still renders both panes as (narrow) columns and both composers stay usable (the shipped behavior — tab-strip mode deferred, DRIFT-1.11).
- **TEST-24** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/14-split-chat/composer-isolation.spec.ts` — asserts: a conversation opened in a second pane binds its sends to its own conversation id (project context follows the focused pane, DRIFT-1.3) independently of the primary pane.
- **TEST-25** (tier: unit) [covers: ITEM-15] file: `src-app/ui/src/modules/chat/core/split/galleryCoverage.test.ts` — asserts: `SplitChatView` + `ChatPaneContext` carry `coverage.ts` `kind:'via'` entries (the gallery-coverage contract that stands in for dedicated multi-pane gallery cells, DRIFT-1.12) so `check:gallery-coverage` is satisfied without a live two-pane SSE cassette.
- **TEST-26** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: with the split feature present but only ONE pane, the legacy single-conversation surface is unchanged (no `chat-pane` wrapper; composer works) — the `Stores.Chat` bridge no-regression path.
- **TEST-27** (tier: unit) [covers: ITEM-16] file: `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` — asserts: the shipped pane-reorder capability — `reorderPanes(from,to)` moves a pane and an out-of-bounds reorder is a no-op (pointer-drag reorder deferred to this store action, DRIFT-1.10).
- **TEST-28** (tier: e2e) [covers: ITEM-16, ITEM-8] file: `src-app/ui/tests/e2e/14-split-chat/independent-input.spec.ts` — asserts: the explicit `Split` button (the shipped keyboard/click affordance; drag-to-split deferred, DRIFT-1.10) opens a new pane.
- **TEST-29** (tier: e2e) [covers: ITEM-17] file: `src-app/ui/tests/e2e/14-split-chat/popout-new-tab.spec.ts` — asserts: the pop-out button (the shipped tear-off equivalent, DRIFT-1.10) detaches a conversation into its own top-level window.
- **TEST-30** (tier: e2e) [covers: ITEM-18] file: `src-app/ui/tests/e2e/14-split-chat/right-panel-per-pane.spec.ts` — asserts: with 2 panes, opening a right-panel tab in a pane renders it as a slide-over INSIDE that pane (not a 3rd column); the other pane keeps its width.

## Audit-gap tests (from the re-audit)

- **TEST-31** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/14-split-chat/composer-isolation.spec.ts` — asserts: per-pane composer isolation for the SHIPPED per-pane stores — draft text (per-pane `TextStore`) and selected model (per-conversation re-key) set in pane A do not appear/apply in pane B (file/assistant/MCP follow focus, DRIFT-1.2/1.3/1.4).
- **TEST-32** (tier: e2e) [covers: ITEM-6, ITEM-2] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: a live frame reaches only its owning pane's store (the `applyStreamFrame` conversation-id filter over the shared `chat:token` bus, DRIFT-1.5) — pane B receives no assistant message during pane A's stream.
- **TEST-33** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/14-split-chat/independent-streaming.spec.ts` — asserts: each pane's stream connection is programmed for its OWN conversation — pane A streaming never reprograms/leaks into pane B's connection.

## Consolidated-audit tests (MCP per-pane, backend cap, nav, connection lifecycle)

- **TEST-34** (tier: e2e) [covers: ITEM-19] file: `src-app/ui/tests/e2e/14-split-chat/composer-isolation.spec.ts` — asserts: MCP/tool routing follows the focused pane (DRIFT-1.4 — full per-pane `McpComposer` deferred): interacting in a pane focuses it so a send uses THAT pane's conversation + composer, verified via the focus-routed model/send isolation.
- **TEST-35** (tier: unit) [covers: ITEM-19] file: `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` — asserts: the per-pane focus model that makes MCP routing correct — `focusPane` sets exactly one focused pane and `setPaneConversation` re-points a single pane's conversation, so a focus-scoped composer read resolves to one pane (the shipped basis for MCP pane-safety, DRIFT-1.4).
- **TEST-36** (tier: integration) [covers: ITEM-20] file: `src-app/server/src/modules/chat/stream/registry.rs` — asserts: (Rust `#[cfg(test)]`) the per-user connection cap reads the configured value (raised default) instead of the hardcoded 12, and the (cap+1)th connection 429s at the configured bound.
- **TEST-37** (tier: e2e) [covers: ITEM-11, ITEM-1] file: `src-app/ui/tests/e2e/14-split-chat/new-chat-adopt.spec.ts` — asserts: creating a conversation in a second (new-chat) pane adopts it into THAT pane and does NOT navigate the whole window away (no window-hijack; the other pane stays put).
- **TEST-38** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/14-split-chat/new-chat-adopt.spec.ts` — asserts: switching a pane's conversation loads a FRESH stream/message state for the new conversation without leaking the old one's buffered frames.

## Tree-fix tests (virtualization + MessageViewState + store-kit)

- **TEST-39** (tier: e2e) [covers: ITEM-2, ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/independent-scroll.spec.ts` — asserts: scrolling pane A to the top virtualizes/paginates A's history WITHOUT paginating or moving pane B; each pane's virtualizer + top-sentinel operate independently.
- **TEST-40** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/independent-scroll.spec.ts` — asserts: the per-pane virtualizer scroll-adjust is independent — pane A's scroll/window-shift does not perturb pane B's mounted window (the module-singleton anchor collision is gone).
- **TEST-41** (tier: unit) [covers: ITEM-21] file: `src-app/ui/src/modules/chat/core/stores/MessageViewState.store.test.ts` — asserts: message collapse/expand + height view-state is scoped to a conversation id, so resetting/switching one conversation's view-state does not clear another's (the per-pane MessageViewState invariant).
- **TEST-42** (tier: unit) [covers: ITEM-22] file: `src-app/ui/src/core/store-kit.test.ts` — asserts: a `defineLocalStore` instance exposes a raw `StoreApi` (`subscribe`/`getState`/`setState`) and two instances' state + subscriptions are independent (the per-pane `ctx.chatStore` foundation).
- **TEST-43** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/14-split-chat/find-per-pane.spec.ts` — asserts: the find affordance is pane-scoped — clicking pane A's own `conversation-find-toggle-btn` opens the find bar in pane A ONLY (each pane owns its `findOpen` state + `ConversationFindBar`); pane B's find bar stays closed.
- **TEST-44** (tier: unit) [covers: ITEM-15, ITEM-23] file: `src-app/ui/src/modules/chat/core/split/galleryCoverage.test.ts` — asserts: the split surface's gallery coverage is satisfied via the `coverage.ts` `via` entries + the `14-split-chat` e2e specs (the backend-free gallery can't seed two live streaming panes, DRIFT-1.12) — the `check:gallery-coverage` contract holds.
- **TEST-45** (tier: e2e) [covers: ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/14-split-chat/new-chat-adopt.spec.ts` — asserts: switching a pane's conversation via the sidebar reloads THAT pane's messages (the `ChatPaneProvider` `loadConversation` effect on `conversationId` change) without disturbing the other pane (DEC-14 in-pane switch).
