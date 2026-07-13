# FIX_ROUND-1 — remediation of the blind-audit ledger

Blind round 1 spawned 4 fresh diff-only agents across 18 angles (correctness,
concurrency, state-management, perf, scale-performance, a11y, responsive-fidelity,
patterns-conformance, precedent-fidelity, affordance-parity, reactive-read-in-loop,
design-in-context, api-contract, tests-quality, error-handling, new-rendering-context,
i18n, security-authz). See LEDGER.jsonl.

## Confirmed → fixed

- **HIGH runaway paging** (api-contract/correctness): `recentHasMore` could stay
  true on a drifted `recentTotal` → infinite empty-page fetches. Fixed with
  server-page-size end-detection + no-progress guard in `loadRecentConversations`.
  Tests: TEST-3b, TEST-3c.
- **MED stuck-spinner regression** (correctness/error-handling): first-load failure
  wedged the widget on the spinner. Fixed with `recentError` + a retryable
  `ErrorState`. Tests: TEST-3d + the `seeded-recent-convos-error` gallery cell.
- **MED offset-skip on delete** (state-management): deleting a loaded row skipped a
  conversation on the next page. Fixed by re-anchoring `recentPage=floor(len/limit)`
  in delete/bulkDelete/sync-delete so dedup recovers it.
- **MED selectedId scroll jank** (perf): O(n)+per-row-hook scan every render →
  `useMemo`.
- **LOW nested live regions** (a11y): removed the outer `role=status/aria-live`.
- **MED/LOW fixed-height clipping + 2px pill drift** (responsive/precedent):
  switched to dynamic `virt.measureElement` (content-height rows) + `pb-0.5` gap.
- **LOW no visible loading caption** (responsive): `Spin description="Loading more…"`.
- **MED vacuous TEST-11** (tests-quality): rewritten to prove windowing by asserting
  the off-screen top row UNMOUNTS after scrolling all rows in (done proactively).
- Stale comment in `ConversationList.tsx` updated (decoupling note).

## Rejected (with rationale — see LEDGER)

- Eager page-fill on a tall viewport — working-as-intended (bounded by viewport/ROW_H).
- Keyboard-can't-reach-beyond-window — inherent to DOM virtualization, shared by
  `kit/table.tsx`; a STRICT improvement over the prior 20-row hard cap; `/chats` is
  the accessible full-list alternative.
- `text-start` vs Menu's `text-left` — `text-start` is lint-mandated for module code
  and the more-correct RTL choice; the Menu's `text-left` is a kit-file exemption.

## Verification

- `npm run check` (ui): PASS. `tsc --noEmit`: clean. Unit tests: 11/11 PASS.
- menuRowClasses extraction confirmed byte-identical by the blind patterns reviewer.

**New confirmed findings:** 2 (the re-audit round after these fixes surfaced two
NEW interaction bugs — see FIX_ROUND-2)
