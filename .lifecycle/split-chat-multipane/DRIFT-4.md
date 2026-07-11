# DRIFT-4 — split-chat-multipane (iteration round 3)

Implementation-vs-plan reconciliation for the round-3 DELTA (ITEM-43, FB-8: the
explicit 3-way open-conversation prompt). Rounds 1–2 already converged.

- **DRIFT-4.1** — verdict: none — ITEM-43(a) the reusable `dialog.choose` kit
  primitive shipped in `components/ui/kit/dialog-host.tsx` exactly as planned:
  additive to the existing imperative `confirm`/alert queue, resolves the chosen
  option key (or null on cancel/dismiss), stamps `${testid}-opt-<key>`. The
  `confirm`/alert paths are byte-identical. Reconciled.

- **DRIFT-4.2** — verdict: none — ITEM-43(b) the PURE `needsOpenChoice` predicate
  shipped in `split/reconcile.ts` (true iff `auto` + `panes>=2` + not-already-open),
  unit-covered by TEST-64. No existing reducer path changed. Reconciled.

- **DRIFT-4.3** — verdict: impl-wins — ITEM-43(c) the `useOpenConversation` branch
  matches the plan (single→collapse+navigate / replace→replaceFocused /
  new→newPane), with ONE addition the plan didn't spell out: the **"Add as a new
  pane" option is hidden when already at `MAX_PANES`** (else it would immediately
  hit the `capReached` path and surface a second dialog). A superset of the plan
  that improves the UX; no PLAN amendment needed. Verified green by TEST-63 (the
  3-pane case starts from a 2-pane split, so "new" is offered). Reconciled.

- **DRIFT-4.4** — verdict: resolved — the behavior change necessarily altered the
  existing `sidebar-reroute.spec.ts` (TEST-50): a plain click on a new conversation
  while split now PROMPTS instead of silently replacing. Updated that spec to click
  "Replace the active pane", preserving its original assertion, and re-ran it green.
  (Mechanical: `stateMatrix.generated.ts` + `STATE_MATRIX.md` regenerated for the
  new dialog render state via `npm run gen:state-matrix`.) Reconciled.

**Unresolved drifts:** 0
