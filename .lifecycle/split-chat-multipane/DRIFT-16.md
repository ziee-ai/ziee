# DRIFT-16 — FB-26 small-screen redesign (ITEM-79..83)

Implementation-vs-plan reconciliation for the small-screen split redesign that
replaces the mobile tab strip + drag chrome with a `PaneManagerDrawer` and makes a
mobile split pane read as a normal single-pane conversation.

- **DRIFT-16.1** — verdict: impl-wins — **ITEM-30 / ITEM-12 (mobile tab strip) is
  SUPERSEDED by ITEM-79..83.** The plan built a `PaneTabStrip` (v2, TEST-23) as the
  small-screen switch UI; live human review (FB-26) found the always-visible strip
  overlaps the fixed sidebar-collapse toggle, and the per-pane header carried
  desktop-only affordances (reorder grip, per-pane ✕, drag-drop) that are nonsense on
  touch, plus a left-inset bug (only pane 0 reserved the toggle inset). The plan is
  amended: the tab strip is deleted; small-screen switching/adding/closing moves to the
  `PaneManagerDrawer`, and a focused pane renders the normal single-pane header. PLAN.md
  ITEM-79..83 added, ITEM-30/12 marked superseded; TEST-23 re-pointed at the rewritten
  `mobile-panes.spec.ts`; `PaneTabStrip.tsx` + `mobile-tabs.spec.ts` deleted. The
  panes-stay-mounted / single-visible-pane guarantee (ITEM-30's real value) is preserved
  — only the CHROME changed. Re-ran `--phase 1..3` after the amendment.

- **DRIFT-16.2** — verdict: none — Desktop is byte-unchanged (DEC-75/DEC-76): columns
  tiling, drag-to-split, reorder grips, one-click split, the 1px divider + imperative
  resize (ITEM-76/77) all run on the `!md` paths, which the diff only WRAPS (never edits)
  in `md`-conditionals. The existing desktop split specs (run at the default desktop
  viewport) remain the coverage for those paths and stayed green in the batch2 suite.

- **DRIFT-16.3** — verdict: none — `paneManagerOpen` is transient by construction: it is
  absent from the store's `snapshot()` (the persisted set) AND from the debounced-save
  `watch()` fingerprint, so toggling the drawer neither persists nor schedules a save
  (DEC-77). Verified by TEST-117 (toggling leaves every persisted layout field
  byte-identical) and by inspection of `SplitView.store.ts` `snapshot()`/`watch()`.

- **DRIFT-16.4** — verdict: resolved — The `PaneManagerDrawer` close ✕ initially called the
  raw `Stores.SplitView.closePane`, which (unlike the header ✕) skips the collapse-to-single
  `reset()` + navigate-to-survivor logic, leaving a stale 1-pane workspace + the full-bleed
  drawer covering the survivor. Reconciled during implementation to route through the shared
  `useClosePane` hook and self-dismiss the drawer when the workspace collapses to a single
  pane — matching the header-✕ behavior (and the FB-25 URL-follows-survivor guarantee).

- **DRIFT-16.5** — verdict: resolved — Blind multi-angle audit (3 fresh agents: correctness/state,
  patterns/reuse, a11y/responsive) found NO HIGH issues on the FB-26 diff. Actioned findings:
  routed `openAnother` through the canonical `useOpenConversationInWorkspace` (killed the
  triplicated seed logic); focus-management on in-list pane close (refocus the first surviving
  row — WCAG 2.4.3); dismiss the drawer on a programmatic collapse; ✕ Tooltip + `size-9`;
  section-label `px-2`; active-row `text-accent-foreground`. Accepted-with-rationale: the
  Open-another search filtering only the loaded ChatHistory page (identical to the sibling
  `ConversationPickerPane`), the search/filter duplication, and the unreachable null-primaryConvId
  path. One incidental PRE-EXISTING desktop bug (closePane divider-width end-truncation) recorded
  out-of-scope. All in `LEDGER.jsonl`.

- **DRIFT-16.6** — verdict: impl-wins — **Two live human bug reports handled (FB-27/FB-28), one
  amending the plan.** FB-27 (open-conversation-choice popup "doesn't work") was diagnosed as a
  DEV-MODE connection-starvation artifact, NOT a defect (logic + production-harness verified) — no
  code change. FB-28 (mobile split panes lack the single-pane auto-hide header + native scroll)
  IS a real gap and amends the plan with **ITEM-84**: `SplitChatView` owns the native-scroll flag,
  the shell relaxes, and the focused pane's stable header `<div>` auto-hides via the new
  `useScrollAwayHeader`. The reconciliation surfaced a subtle crash (conditional store-proxy read =
  a conditional hook) fixed by reading `Stores.AppLayout.nativeScroll` unconditionally (DEC-78).
  Re-ran `--phase 1..4` after adding ITEM-84 / DEC-78 / TEST-118.

- **DRIFT-16.7** — verdict: resolved — Human review of the FB-28 fix ("there is a reason the header
  has top:5 and a block of relative render on top"). The first pass RE-DERIVED the auto-hide (a
  `useScrollAwayHeader` hook + `sticky top-0`), throwing away `HeaderBarContainer`'s deliberate
  `top:5` (iOS under-notch latch dodge) + the safe-area backdrop. Since the actual focus-switch
  crash was the conditional store-proxy read (fixed), the `<div>`↔`HeaderBarContainer` swap is safe
  — so the focused mobile pane now renders the real `HeaderBarContainer` and the re-derived hook was
  DELETED. Same rule as FB-18 (reuse the sibling; don't re-derive its magic values). Re-verified:
  auto-hide pins at top:5 (sticky), wipes on scroll-down; switch/close 16/16, no crash.

**Unresolved drifts:** 0
