# DRIFT-9 — split-chat-multipane (round 7: header-chrome-per-context audit fixes)

Implementation-vs-plan reconciliation for round-7 (FB-13 → ITEM-55/56). Built after
an action audit that DROVE every context-sensitive chat button in single-pane /
split pane / pop-out window and measured its behavior.

- **DRIFT-9.1** — verdict: none — ITEM-55/56 shipped as planned: the back arrow is a
  pure render-gate (`!isSplit && !isPopoutWindow`) in TitleEditor; the pop-out window
  additionally hides the split button (ConversationPage gate) + the pop-out button
  (`popoutActionVisible` third param). Shared `useIsPopoutWindow()` is the single
  route source. Every fix RUN by a test (TEST-65b pure + TEST-85 real-DOM e2e).

- **DRIFT-9.2** — verdict: none — the audit confirmed the OTHER context-sensitive
  actions are already correct: find + edit-title are per-pane (driven in pane B →
  affect pane B only), the composer (send/＋/KB/MCP/voice) is per-pane (prior rounds),
  and pane ✕/grip are pane-scoped. No drift; the audit's only defects were the three
  window-management buttons, all fixed.

- **DRIFT-9.3** — verdict: resolved — a mechanical drift: TEST-84 (the round-6
  blind-audit HIGH-fix test for `snapBackAsNewPane`) existed + passed but had never
  been enumerated in TESTS.md; added its entry (and TEST-85). `stateMatrix.generated`
  regenerated for the new conditional-render states (the gated back/split buttons).
  Reconciled; `npm run check` green both workspaces.

**Unresolved drifts:** 0
