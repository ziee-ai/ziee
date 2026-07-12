# DRIFT-11 — split-chat-multipane (audit round: paper-9/9 correction)

An independent completeness audit found the prior 9/9 was a PAPER-9/9: real per-pane
bugs shipping unfixed + HOLLOW tests (a passing test line that never exercised its
claimed behavior). This round fixes ALL 11 items with REAL covering tests (each RUNS
the behavior across two ACTIVE panes, rule B7), and records the gaps honestly rather
than absorbing them. Reconciliation:

- **DRIFT-11.1** — verdict: impl-wins — DEC-30 "clean cut" OVERSTATED completion:
  `useChatStore` still exists (~7 files) and `Stores.Chat` is a focused-pane bridge/
  shim, not a removed singleton. It works (pane-rebound SSE + `ownerPaneId` hooks) but
  the DEC claimed a removal that did not happen. Amended DEC-30 honestly (bridge-shim
  architecture, not a clean cut); finishing the cut is future work, out of scope here.

- **DRIFT-11.2** — verdict: plan-wins (fixed) — four HIGH per-pane bugs shipped under
  the prior 9/9, contradicting the plan's per-pane isolation intent (and PLAN.md's
  "main's new modules are already split-safe", false for the skill drawer): the skill
  drawer global singleton (ITEM-59), the un-gated Cmd-F window listener (ITEM-60), the
  TitleEditor focused-pane save (ITEM-61), and the text-draft focused-pane + module-
  global key (ITEM-62). All fixed + real covering tests (TEST-94..97). These were the
  code being wrong; the plan's per-pane intent wins.

- **DRIFT-11.3** — verdict: resolved — five HOLLOW tests: TEST-53 (MCP approval leg
  punted to a unit), TEST-58 (phantom workflow-card export claim), TEST-56 (literature-
  reason + same-file view-state piggybacked), TEST-67 (voice close with no close
  action), plus the systemic streaming-isolation methodology (idle-empty-pane control).
  Each now has a REAL exercising test (TEST-98..104). The workflow card (ITEM-64) was
  already pane-correct in code — the drift was the phantom test CLAIM, corrected.

- **DRIFT-11.4** — verdict: none — the audit CONFIRMED as genuinely REAL (not re-done):
  per-pane ChatPaneStore + independent streaming into the acting pane, per-pane
  composer draft/model/file/assistant + send-blocker, per-pane scroll/virtualizer,
  keyboard scoped to `focusedPaneRoot()`, per-pane right-panel open/close, message
  copy/edit/regenerate/branch via `useChatPaneOrNull`, KB grounding + citation
  highlight per-pane, voice recording lock, tool-approval routing (code-correct),
  pop-out-MOVES-out, drag-to-split. Descoped web/lit/bio/admin surfaces legitimately N/A.

- **DRIFT-11.5** — verdict: resolved — unproven-in-CI marked EXPLICITLY (not claimed as
  CI-passing proof): cross-window snap-back (TEST-81..84) + tear-off native-window
  positive (TEST-93) are desktop-host-only (Tauri emit/listen + real WebviewWindow),
  recorded in FB-15/FB-16 + PLAN_AUDIT.

**Unresolved drifts:** 0
