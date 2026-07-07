# DRIFT-1 — implementation vs plan

Reviewed the full working tree against PLAN.md / DECISIONS.md after implementing
ITEM-1..10. Cross-checked: `tsc` green in BOTH workspaces, `npm run check` green
in BOTH, all 13 unit tests green (6 new proxy specs + TEST-9 + existing
tool-status suite proving the loader is non-breaking), guardrail empirically
flags `.__state`, state-matrix regenerated with zero residual `.__state`.

- **DRIFT-1.1** — verdict: none — ITEM-2: the second (`getState()`-only) arm of `ExtractZustandState` previously exposed `__state` but not `$`; I removed `__state` and added `$` there so `$` is uniformly available on both arms. This is exactly ITEM-2's intent ("leaving only `$`") — reconciled, not a divergence.
- **DRIFT-1.2** — verdict: none — the two unit specs must reference the removed alias to prove it's gone; they use bracket access (`p['__state']`) which hits the same proxy trap but is not a `.member` expression, so the grit ban stays strict (no test-dir exclusion was weakened). A test-authoring detail, within ITEM-8/ITEM-10 scope.
- **DRIFT-1.3** — verdict: none — MIXED sweep files (chrome.tsx, LiteratureScreeningPanel.tsx) used explicit per-token maps and the two multiline `Stores.Workflow.__state\n.test(` sites used a "drop the bare alias" transform; both realize the DEC-7 rule (action→direct, field→`$.field`) exactly. No blanket `.__state.`→`.` was applied to a field (that hazard was caught and avoided). Verified by diff review + green tsc.
- **DRIFT-1.4** — verdict: none — no line-count changes in any swept component (all edits are in-place text swaps), so the regenerated `stateMatrix.generated.ts` diff is confined to the handful of conditional-render condition strings that embedded `.__state` (now `.$.`), at stable line numbers. `check:state-matrix` green confirms no drift.

**Unresolved drifts:** 0
