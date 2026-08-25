# DRIFT-1 — implementation vs plan / design invariants

Authored live during phase 5 as each item landed. Reconciled every ITEM against
PLAN.md AND the `## Invariants`.

- **DRIFT-1.1** — verdict: none — ITEM-1 (`shrink-0` on the card) matches the plan
  and DEC-5; the kit `Card`'s `overflow-hidden` is kept. Root cause proven live
  (offset 58 vs scroll 152). Upholds INV-1.
- **DRIFT-1.2** — verdict: none — ITEM-2 (kind icon + `line-clamp-2` title +
  inline status Tag + compact muted meta row) matches the plan and mirrors
  `SubAgentActivityCard`; tokens/kit only (kit `Card`/`Tag`/`Text`, lucide icons),
  no raw element, no hardcoded color. Upholds INV-1/INV-3.
- **DRIFT-1.3** — verdict: none — ITEM-3 ships the kit `Tabs variant="line"
  size="sm"` region (Transcript default + Result) reading the already-exposed
  `detail.activity`, reusing the shared `AgentActivityTimeline` and
  `BackgroundRunResult`. Matches DEC-1/2/3. Upholds INV-2/INV-3.
- **DRIFT-1.4** — verdict: resolved — the plan said the expander test-id would be
  renamed; implemented as `background-run-details-toggle-${id}` (was
  `background-run-result-toggle-${id}`). The ONE spec that used the old id
  (`background-transcript.spec.ts`) is updated in the same round (ITEM-5), and the
  result-body test-ids (`background-run-final-text-${id}`) are preserved under the
  Result tab. No other spec referenced the old toggle. Reconciled, no plan change.
- **DRIFT-1.5** — verdict: resolved — TEST-5 was re-scoped from "mount the full
  card" to "mount the exported pure `BackgroundRunDetailTabs`" because the full
  card reads the BackgroundRuns store (fragile in jsdom); the sub-component is
  props-only and proves the same tab/transcript/result behavior. TESTS.md TEST-5
  wording was amended accordingly (phase-3 gate re-run green). `impl-wins` on the
  test shape; the invariant coverage is unchanged (INV-3 still e2e-pinned by
  TEST-3, INV-2 by TEST-2).

**Unresolved drifts:** 0
