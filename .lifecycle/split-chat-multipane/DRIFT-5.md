# DRIFT-5 — split-chat-multipane (iteration round 4)

Implementation-vs-plan reconciliation for the round-4 DELTA (ITEM-44, FB-9 /
DEC-60: single-pane pop-out is desktop-only). Rounds 1–3 already converged.

- **DRIFT-5.1** — verdict: none — ITEM-44 shipped exactly as planned: a pure
  `popoutActionVisible(inPane, isDesktop)` in `chat/core/popout/popoutVisibility.ts`
  (own module + unit test TEST-65) + a one-line render gate in
  `OpenInNewWindowAction.tsx` using the existing runtime `'__TAURI__' in window`
  check. Split panes unaffected (both platforms still show it); single-pane hides
  on web, shows on desktop. Desktop shares ui's component (no copy), so the runtime
  check covers both bundles. Reconciled.

- **DRIFT-5.2** — verdict: resolved — re-pointing was required: the old
  `popout-new-tab.spec.ts` popped out from single-pane WEB, which this change now
  hides. Rewrote it to (a) assert the single-pane-web-hidden / split-present gate
  (TEST-66) and (b) pop out from a SPLIT pane, preserving the independent-tab +
  original-usable assertions (TEST-P3/P4) and additionally showing the pane
  moves out. TESTS.md TEST-P3/P4 descriptions updated to the split context; no
  TEST-ID dropped (A5). Mechanical: `stateMatrix.generated.ts` + `STATE_MATRIX.md`
  regenerated for the new conditional-render (hidden) state. Reconciled.

**Unresolved drifts:** 0
