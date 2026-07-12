# FIX_ROUND-19 — split-chat-multipane (independent completeness audit)

The blind adversarial review this round was an INDEPENDENT completeness audit (external
auditor, diff + full-codebase read) that found the prior 9/9 was a PAPER-9/9: real
per-pane bugs shipping unfixed + HOLLOW tests. All findings confirmed against the code
and FIXED, most-severe first, each with a REAL covering test (rule B7 — MOUNT + assert
across two ACTIVE panes; where a boundary is mocked, the pane→conversation routing crux
is real). Full findings + dispositions in `LEDGER.jsonl`; per-item verdicts in PLAN_AUDIT.

## Fixed (confirmed)

- **HIGH** #1 skill drawer global singleton (ITEM-59, TEST-94) · #2 Cmd-F window-global
  (ITEM-60, TEST-95) · #3 TitleEditor focused-pane save (ITEM-61, TEST-96) · #4 text
  draft focused-pane + module-global key (ITEM-62, TEST-97).
- **MED** #5 MCP approval-routing e2e (ITEM-63, TEST-98) · #6 workflow-card export +
  corrected phantom claim (ITEM-64, TEST-99) · #7 same-file view-state isolation
  (ITEM-65, TEST-100) · #8 voice close-during-record (ITEM-66, TEST-101) · #9 find
  search-scope (ITEM-67, TEST-102).
- **LOW** #10 EditingMessageBanner + CanvasSelectionPopover (ITEM-68, TEST-103).
- **SYSTEMIC** #11 two-simultaneous-streams bidirectional isolation (ITEM-69, TEST-104).
- **Plan overstatement** DEC-30 "clean cut" → amended to impl-wins drift (DRIFT-11.1).

## Recorded honestly (not absorbed)

- The paper-9/9 finding itself → PLAN_AUDIT "Independent completeness audit" section +
  DRIFT-11 + FB-16.
- Unproven-in-CI (TEST-81..84 cross-window snap-back + TEST-93 tear-off native positive)
  marked desktop-host-only, NOT CI-proof (FB-15/FB-16, PLAN_AUDIT).
- Audit-confirmed-REAL surfaces NOT re-done (DRIFT-11.4).

**New confirmed findings:** 12
