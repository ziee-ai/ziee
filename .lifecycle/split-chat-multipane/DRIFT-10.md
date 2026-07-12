# DRIFT-10 — split-chat-multipane (round 8: single-pane edge-drop + desktop tear-off)

Implementation-vs-plan reconciliation for ITEM-57 (single-pane edge-directional drop)
and ITEM-58 (desktop tear-off). Both were built test-first with the behavior PROVEN by
running (pure node:test + real-DnD e2e), per the standing rules.

- **DRIFT-10.1** — verdict: none — ITEM-57 shipped as planned: pure `zoneForX` +
  `planSinglePaneDrop` (left → `[dropped|current]`, right → `[current|dropped]`, center →
  replace, self/empty → noop), wired into `ConversationPage`'s single-pane chat column with
  a token-styled left/center/right hint overlay. Executes via `SplitView.openPane` (split,
  explicit seed order) + `useOpenConversationInWorkspace` (center replace). RUN by TEST-88/89
  (unit) + TEST-90 (e2e — all three zones by aimed clientX).

- **DRIFT-10.2** — verdict: none — ITEM-58 shipped as planned: pure `isOutsideWindow` +
  `planTearOff` + `runTearOffPlan`, wired via `useConversationTearOff` onto the `onDragEnd`
  of the sidebar (`RecentConversationsWidget`, `ConversationCard`) + the split pane grip.
  Desktop-only + strict-past-the-edge gate lives in the pure decision; a pane source MOVES
  (closePane). Reuses the ⤢ button's `openConversationWindow` seam + ITEM-29 MOVE verbatim.
  RUN by TEST-91/92 (unit + spied exec glue) + TEST-93 (e2e web desktop-only gate).

- **DRIFT-10.3** — verdict: resolved — a PLAN-COVERAGE correction (not a code drift): the
  original ITEM-16 (in-tile edge drop-zones Split-left/Replace/Split-right) had been reduced
  to only `reorderPanes` (TEST-27, "edge-drop deferred"), and ITEM-17 (desktop tear-off) was
  mapped to TEST-28 (`drag-to-split.spec.ts`), which does NOT exercise tear-off — so both
  were "covered on paper" at the prior 9/9 but never genuinely implemented. ITEM-57 now
  genuinely implements the single-pane edge-drop half of ITEM-16 (the split view's edge
  cases already ship via ITEM-31 header=replace + seam=new-pane); ITEM-58 genuinely
  implements ITEM-17. Recorded as FB-14 + the PLAN_AUDIT "Plan-coverage correction" section,
  surfaced to the human rather than silently absorbed. The old TEST-27/28 mappings stay
  valid (they test what they test); TEST-88..93 are the genuine coverage.

- **DRIFT-10.4** — verdict: none — desktop parity: the desktop workspace reuses `ui/src`
  via the vite `fallbackSrc`/`srcDirs` alias, so the new hook + helpers + `ConversationPage`/
  widget edits land once in `ui/src` and both `npm run tsc` (ui + desktop/ui) pass. No new
  `.desktop.ts` seam — tear-off reuses the existing `openConversationWindow.desktop.ts`; the
  desktop-only gate is a runtime `'__TAURI__' in window` check, not a build seam.

**Unresolved drifts:** 0
