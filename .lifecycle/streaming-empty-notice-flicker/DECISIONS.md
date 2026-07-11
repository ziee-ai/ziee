# DECISIONS — all resolved up front (no open markers)

### DEC-1: Root-cause fix vs. notice-only gate?
**Resolution:** Do BOTH — the atomic streaming→persisted handoff (root cause of the
disappear/reappear AND the false notice) PLUS a defensive `finalizing` notice gate.
**Basis:** user — the human explicitly chose "Atomic handoff + gate" when asked. The gate alone
would leave the content disappear/reappear the task also requires gone.

### DEC-2: How to keep one source of truth for the tail merge — extend `reconcileTail`, or a pure helper?
**Resolution:** Extract a pure `finalizeTailWindow` helper in `messageWindow.ts` and inline
getHistory + the atomic `set()` in the `complete` handler; leave `reconcileTail` untouched.
**Basis:** codebase — `reconcileTail` is also used by the `started` receiving-device path; threading
a terminal-state option through it (and through `loadMessages` for the branch-changed path) is more
invasive and risks that path. The pure helper is unit-testable and additive (lowest blast radius).

### DEC-3: Is `finalizingTurn` admin-configurable or a fixed transient?
**Resolution:** Fixed transient store flag (not a setting). It is an internal render-timing signal,
not an operational tunable — no memory/CPU/timeout/retention/quota semantics.
**Basis:** convention — the configurable-settings rule targets operational tunables; a sub-second UI
state flag (sibling of `branchChangedDuringStream`/`isStreaming`) is correctly a fixed constant.

### DEC-4: Does #135 approval-scroll need a code change, or only verification?
**Resolution:** Verify-first. The `ConversationPage.tsx:318-363` one-shot should become robust once
the finalize no longer remounts/flickers the row. Add a minimal re-assert guard ONLY if Phase-8
live/e2e observation shows a residual re-measure race (approval settles below the fold after the
one-shot). This diff does not touch `ConversationPage.tsx`, so ITEM-6 is covered by keeping the
EXISTING #135 approval-scroll spec green (TEST-5) plus live verification.
**Basis:** codebase — the scroll effect keys off `Stores.McpComposer.toolCalls` + a page-level
dedup `Set` (stable across child remounts), so a remount cannot re-consume it; the only residual
risk is a post-scroll height re-measure, which the atomic handoff removes.

### DEC-5: Add an isolated e2e for mid-stream cancel + background-convo completion (ITEM-7)?
**Resolution:** No isolated new e2e; cover ITEM-7 by (a) leaving the cancel/error/background
null-sites byte-identical except the on-screen `complete` path, (b) the Phase-6 state-management
audit angle, and (c) keeping `empty-completion.spec.ts` green (cancel → `interrupted`, notice
suppressed).
**Basis:** convention — mocking a mid-stream abort + a background-conversation race in Playwright is
high-cost/low-signal; the change to those paths is nil, so the regression surface is the on-screen
path already covered by TEST-3/TEST-4.

### DEC-6: Widen the handoff gap in the e2e deterministically — how?
**Resolution:** Delay the mocked `GET .../messages` (getHistory) response in TEST-3 by a fixed delay
via the `page.route` fulfil, so the post-`complete` finalize awaits long enough to poll the DOM for
"text present + notice absent" during the gap. On `origin/khoi` this window renders the empty/absent
frame (spec fails); with the fix the row is continuously present (spec passes).
**Basis:** codebase — `sse-mock-helpers.ts` already intercepts these routes with `page.route`;
adding a delay to the getHistory fulfil is the same mechanism `empty-completion.spec.ts` uses to
stage `mockGetMessages`.
