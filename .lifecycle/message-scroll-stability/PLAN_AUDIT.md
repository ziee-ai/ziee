# PLAN_AUDIT — message-scroll-stability

Audited against the worktree HEAD (`b1802c26`, post `file-viewer-virtualization`).

## Breakage risk

- **MessageList imperative handle (`MessageListHandle`)** is consumed by
  `ConversationPage.tsx` (`scrollToMessageId`/`scrollToBottom`/`captureAnchor`/
  `restoreAnchor`). ITEM-7 ADDS a method (`anchorAcrossResize` or similar); it must
  not change the existing four signatures. Additive → no caller breakage. Verified
  the interface is re-created via `useImperativeHandle` deps `[virt, virtualize, count, scrollerReady]`; adding a method keeps those deps valid.
- **InlineFilePreview** is rendered by `MessageFilesView.tsx` per `resource_link`.
  ITEM-2/3/5 change its internals only (body wrapper height, a handle, state source);
  its props (`viewer`/`source`/`file`) are unchanged → no caller breakage. The
  `data-testid`s it emits (`inline-file-preview`, `-body`, `-chevron`) must be kept
  (e2e in `07-mcp`/file specs assert them) — new testids only ADDED.
- **CollapsibleBlock** is used only by `ChatMessage.tsx`. Moving `collapsed` to the
  store keeps the component's public shape (`children`,`maxHeightPx`,`className`,
  `data-testid`) — but it now needs a stable key (the message id). ChatMessage already
  has `message.id`; passing it is additive. Risk: a CollapsibleBlock used WITHOUT a
  key (none today) must fall back to local state — handled by making the store key
  optional (uncontrolled fallback). No breakage.
- **Fixed-height bodies (ITEM-2)** change UX: a short inline file now shows in a
  capped scroll box instead of hugging content (accepted by the requester; softened
  by the resize handle). No API/behaviour contract broken; visual-regression baseline
  WILL shift for the inline-preview gallery cell → re-bless in Phase 8 (expected).
- **`window.__MSGLIST_METRICS__` (ITEM-1)** is `import.meta.env.DEV`-only → compiled
  out of production; zero runtime cost or surface in release. Mirrors the existing
  `window.__GALLERY_OVERLAYS__` dev hook.

## Pattern conformance

- **ITEM-6 store** conforms to the chat-core store idiom: `defineStore` from
  `@/core/store-kit`, state maps + hook-free actions, read-in-handler via `Stores.X.$`
  ([[feedback_stores_state_in_handlers]]). Mirrors `Chat.store.ts`'s
  `conversationStateCache` Map + reset-on-switch. Reset must be wired at the SAME
  site Chat.store clears `messages` (conversation switch) so the two never drift.
- **ITEM-2** conforms to the established in-tree `contain-intrinsic-size` /
  definite-height precedent (`RawCodeView.tsx`, `DelimitedTable.tsx`, `ReservedImage.tsx`)
  from the file-viewer-virtualization feature — not a new invention.
- **ITEM-3** resize handle reuses the existing right-panel width-drag gesture shape
  (pointer capture → persisted px), not a bespoke abstraction
  ([[feedback_check_library_before_custom]]).
- **ITEM-7** reuses MessageList's own `captureAnchor`/`startReassert` machinery +
  `scrollAnchor.utils.ts` pure helpers; the new helper lands in that same pure module
  (unit-testable, matching `scrollAnchor.utils.test.ts`).
- **Design tokens**: any new chrome (resize handle, skeleton) uses semantic tokens
  (`bg-muted`, `border-border`, `text-muted-foreground`) per DESIGN_SYSTEM.md — no raw
  colors. The skeleton reuses the kit skeleton/`animate-pulse` idiom.

## Migration collisions

- **None.** This is a pure frontend change — no `src-app/server/migrations/` files
  touched, no new migration number. `ls migrations/` is irrelevant to this diff.

## OpenAPI regen

- **Not required.** No backend type, route, or response shape changes; no
  `openapi.json` / `api-client/types.ts` regeneration. The store keys off the existing
  `MessageWithContent.id` and `ResourceLink.uri` fields already in the generated types.
  Desktop UI is unaffected (the chat module does not exist in `src-app/desktop/ui`),
  so only the `ui` workspace `npm run check` applies.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive dev-only instrumentation + a new gallery cell; mirrors `window.__GALLERY_OVERLAYS__` and existing seeded chat cells; introduces a new gallery state → covered by `check:state-matrix`/`check:gallery-coverage` (budgeted in Phase 8).
- **ITEM-2** — verdict: PASS — internal to InlineFilePreview; follows the in-tree reserved/definite-height precedent; visual baseline reshoot expected (not a regression).
- **ITEM-3** — verdict: CONCERN — a drag gesture is the one piece with a11y + touch nuance: must expose a keyboard-resizable affordance (arrow keys) + `role`/`aria-label` and not trap pointer on touch; mitigated by mirroring the vetted right-panel width-drag and adding keyboard support. Non-blocking.
- **ITEM-4** — verdict: PASS — lifts local state to a keyed store; uncontrolled fallback preserves the no-key case; no caller breakage.
- **ITEM-5** — verdict: CONCERN — persisting `seen` means a body mounts on remount without a viewport check; with ITEM-2's reserved height this is zero-delta, but must confirm off-screen previews on FIRST load still defer their fetch (initial-scroll-to-bottom path) — the store starts empty per conversation so first load is unaffected. Non-blocking; covered by a test.
- **ITEM-6** — verdict: PASS — new store mirrors `conversationStateCache`; reset wired at the existing window-clear site; no collision.
- **ITEM-7** — verdict: CONCERN — the trickiest: an in-place anchor must cancel/serialize with `startReassert` and the prepend anchor-restore so they don't double-adjust. Mitigated by routing every intentional height change through the single new method that first `cancelReassert()`s. Non-blocking; covered by anchor tests.

No BLOCKED verdicts. The three CONCERNs are design-nuance flags, each mitigated in-plan and pinned to a Phase-3 test.
