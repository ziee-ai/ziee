# TESTS — explicit enumeration (every ITEM ↔ ≥1 TEST)

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/chat/core/stores/messageWindow.test.ts` — asserts: `finalizeTailWindow` yields exactly ONE assistant row and never an empty window — (a) synthetic placeholder id NOT in the tail page is dropped and the persisted real-id row is appended at the tail; (b) real streaming id present in the tail page collapses to a single in-place row (no duplicate); (c) older already-loaded rows are preserved and order is stable.
- **TEST-2** (tier: unit) [covers: ITEM-4, ITEM-5] file: `src-app/ui/src/modules/chat/components/emptyCompletion.test.ts` — asserts: `shouldShowEmptyCompletionNotice` returns false when `finalizing===true` even for an empty non-streaming assistant turn; still returns true for a genuinely-empty COMPLETED turn (`finalizing===false`, not streaming, not interrupted, no visible answer); unchanged for the streaming / user / interrupted / has-answer rows.
- **TEST-3** (tier: e2e) [covers: ITEM-1, ITEM-3] file: `src-app/ui/tests/e2e/chat/streaming-handoff-no-flicker.spec.ts` — asserts: streaming a NORMAL text answer (started + content-with-text + complete) with a GATED `getHistory` (the post-`complete` reconcile blocks on a promise, holding the handoff window open) keeps the assistant answer continuously visible AND `chat-empty-completion-notice` count stays 0 throughout (assert the DOM effect during the open gap, not a spy). On `origin/khoi` the row is deleted before getHistory, so the "answer still visible" assertion FAILS (revert-check); with the fix it PASSES.
- **TEST-5** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: (EXISTING #135 spec, must stay green) a pending tool-approval below the fold (user NOT at bottom) is scrolled into view (`toBeInViewport`). This diff does NOT modify `ConversationPage.tsx` (the approval-scroll owner), so ITEM-6 is a regression guard that removing the streaming flicker/remount did not break the #135 scroll; run it in Phase 8 + confirm live.
- **TEST-4** (tier: e2e) [covers: ITEM-5, ITEM-7] file: `src-app/ui/tests/e2e/chat/empty-completion.spec.ts` — asserts: (EXISTING spec, must stay green) a genuinely-empty completion still shows `chat-empty-completion-notice` after `complete` AND again after `page.reload()` (content-derived, reload-robust), guarding that the fix does not suppress the real notice.

## Tier rationale

- Frontend diff (`src-app/ui/**`) → **≥1 `tier: e2e` required**: TEST-3 + TEST-4 satisfy it.
- No new permission introduced → no `[negative-perm]` restricted-user e2e required (A10 n/a).
- The atomicity (single-`set()`, no intermediate render) is fundamentally a render-timing property,
  so the load-bearing proof is the e2e TEST-3 (widened gap + effect assertion); the unit tests
  (TEST-1/TEST-2) lock the pure pieces (`finalizeTailWindow` invariants + the gate matrix).
- ITEM-7 regression (cancel shows partial; background completion doesn't clobber on-screen flag) is
  covered by keeping the existing `empty-completion.spec.ts` behavior (cancel → interrupted, not
  empty-notice) green via TEST-4 plus the Phase-6 audit's state-management angle; no new isolated
  cancel e2e is added (mocking a mid-stream cancel + background-convo race in Playwright is
  high-cost/low-signal vs. the audit + unit coverage — recorded as DEC-5).
