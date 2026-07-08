# DRIFT-1 — implementation vs plan (message-scroll-stability)

Audited the implemented diff against PLAN.md / TESTS.md / DECISIONS.md after the
first full implementation pass. tsc (ui) clean; 25 unit tests green; guardrails +
color lints green.

## Per-item conformance

- **ITEM-1** — implemented: `window.__MSGLIST_METRICS__` DEV-only correction
  counter in MessageList (`onChange(sync=false)` = a recorrection) + the
  `seeded-message-list-long` gallery surface (`MessageListLongDemo.tsx`, 500
  mixed messages driving the real virtualizer). Matches plan.
- **ITEM-2** — implemented: InlineFilePreview body is a DEFINITE-height
  `overflow-auto` box (`inlineFileHeight.ts`); the `!seen` skeleton uses the SAME
  resolver so the body-mount is zero-delta. Matches plan.
- **ITEM-3** — implemented: bottom `role="separator"` drag handle + keyboard
  (Arrow/Home/End) + persisted height. Matches plan (see DRIFT-1.3 on the
  mirror source).
- **ITEM-4** — implemented: collapse lifted to `MessageViewState` keyed by
  message id; ChatMessage threads `message.id`; uncontrolled local fallback when
  absent. Matches plan.
- **ITEM-5** — implemented: InlineFilePreview collapsed/seen/height lifted keyed
  by `source.url`; `seen` persistence keeps first-load lazy-fetch (DEC-10).
  Matches plan.
- **ITEM-6** — implemented: `MessageViewState.store.ts` + helpers; registered in
  `module.tsx` + typed in `types.ts`; reset wired at the conversation-switch
  clear site AND the store-destroy site (DRIFT-1.4). Matches plan.
- **ITEM-7** — implemented as a self-contained `useInPlaceAnchor` hook +
  `inPlaceAnchorDelta`/`findScrollParent` pure helpers (DRIFT-1.2).

## Drifts

- **DRIFT-1.1** — verdict: impl-wins — TEST-11 was planned against a full-app
  `11-chat/chat-scroll-anchor.spec.ts` asserting BOTH prepend-anchor and
  jump-to-message. Retargeted to the backend-free gallery spec
  (`visual/chat-scroll-stability.spec.ts`), asserting jump-to-message lands +
  settles. Rationale: the prepend-anchor path (`captureAnchor`/`restoreAnchor`/
  `indexRestoreOffset`/`anchorRestoreNeeded`) is UNCHANGED by this diff (verified
  — those functions are untouched) and is already covered by the scrollAnchor
  unit tests, so it needs no new full-app e2e; the gallery surface can drive the
  jump path directly. TESTS.md amended.

- **DRIFT-1.2** — verdict: impl-wins — PLAN ITEM-7 originally said "expose an
  imperative anchor method on MessageList." Implemented instead as a
  self-contained `useInPlaceAnchor` hook the height-owning children
  (`CollapsibleBlock`, `InlineFilePreview`) call directly. Rationale: the height
  change ORIGINATES in those child components; a MessageList method would require
  threading a ref/context across the chat↔file module boundary for no behavioural
  gain. The hook finds its own scroll parent and uses the same pure guard, so it
  cannot double-adjust against the virtualizer. PLAN.md ITEM-7 + Files-to-touch
  amended (added `useInPlaceAnchor.ts`).

- **DRIFT-1.3** — verdict: impl-wins — PLAN ITEM-3 said the resize handle would
  "mirror the right-panel width-drag." Implemented as a self-contained
  pointer+keyboard handler inside InlineFilePreview instead. Rationale: the
  right-panel width drag is not exposed as a reusable primitive; a local
  `setPointerCapture` handler + keyboard steps is the minimal, a11y-complete
  (DEC-6) implementation and avoids inventing a shared abstraction for one
  call-site. Behaviour matches the plan intent.

- **DRIFT-1.4** — verdict: none — PLAN said reset "at the existing clear-window
  site." Implementation also resets on store `__destroy__` (logout / full clear).
  This is a strict superset (no behaviour lost), so it is a reconciled addition,
  not a divergence. PLAN.md Files-to-touch note updated to say "+ on store
  destroy."

- **DRIFT-1.5** — verdict: none — `InlineFilePreview` (file module) now imports
  `messageViewState.helpers` + `useInPlaceAnchor` from the chat module. The file
  chat-extension is already chat-coupled (it imports `@/modules/chat/core/...`
  and `Stores.Chat`), so this adds no new dependency direction.

**Unresolved drifts:** 0
