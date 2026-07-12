# FIX_ROUND-18 — split-chat-multipane (round-8 convergence re-audit)

A fresh BLIND diff-only reviewer re-checked the FIX_ROUND-17 fix commit
(`3fba9f1d9` — the `openConversationInWorkspace('newPane')` split refactor, the
`isOutsideWindow` degenerate-rect guard, and the two e2e updates) for any NEW
defect the fixes introduce.

## Verdict: the fixes converge cleanly — no new defect.

Traced against the real implementations (`useOpenConversation.ts`,
`SplitView.store.ts`, `reconcile.ts`, `singlePaneDrop.ts`, `tearOff.ts`,
`useConversationTearOff.ts`, `openConversationWindow.ts`, `focusPopoutWindow.ts`):

- **`.then()` + left-reorder — CORRECT.** `useOpenConversationInWorkspace` is a real
  async callback so `.then()` is valid; if `focusPopoutWindowIfOpen` short-circuits
  (desktop dedup, no pane created) `findIndex` yields `-1` and the `idx > 0` guard
  skips the reorder (and `reorderPanes` no-ops out-of-range anyway); the synchronous
  store `set` means `$.panes` in the `.then()` is up to date; the URL effect is
  loop-guarded so there's no navigate↔focus loop.
- **`isOutsideWindow` guard — CORRECT.** `!(outerWidth > 0)` returns false for 0 /
  negative / NaN; `Number.isFinite` covers NaN/±Inf on all four coords; it only ADDS
  early-false returns for degenerate rects and strictly REDUCES spurious tear-offs —
  no previously-correct case changes.
- **e2e changes — SOUND, not flaky.** URL asserts match the dropped conversation per
  block; the faked-`__TAURI__` positive control only flips the hook's `isDesktop`
  gate while the web build still resolves the web `openConversationWindow` →
  `window.open` (stub-recorded), honestly proving source→onDragEnd→hook→plan→seam and
  explicitly disclaiming the native `WebviewWindow` path.

Two minor notes were raised and both are explicitly NOT new defects: (1) the tear-off
test's own `OUTSIDE`/`insidePoint` arithmetic already presumes a sane non-degenerate
rect (a pre-existing test precondition, not something the guard breaks; real Chromium
reports sane values); (2) `void …then()` has the same no-`.catch` posture as the
sibling replace branch and every other call site, and the `.then` body can't throw —
no new rejection class. Logged as no-change observations.

**New confirmed findings:** 0
