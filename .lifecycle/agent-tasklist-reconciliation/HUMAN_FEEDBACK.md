# HUMAN_FEEDBACK — agent task-list terminal-state reconciliation

No human feedback received.

This is an autonomous bugfix run (defect mined from the live audit rig); there was
no interactive human review of the running feature. The owner sign-off surface is
the design invariants and their acceptance tests:

- **INV-1** (no open task row survives a terminal run) → TEST-1 (PASS; mutation-RED
  without the hook).
- **INV-2** (completed never rewritten; unfinished never marked done) → TEST-2 (PASS,
  discriminating).
- **INV-3** (every terminal path: cancel + crash-recovery, not just completion) →
  TEST-3 (PASS) + the `sweep_at_boot` wiring test.
- **INV-4** (no orphaned task rows after a run delete) → TEST-4 (PASS, real cascade).

Reported disproof / scope note for the owner: the plan's Part-2 "add a FK on
`run_id`" was DISPROVED as literally stated — `run_id` is polymorphic (chat =
assistant message id, workflow/background = `workflow_runs.id`, fan-out = a fresh
non-persisted id), so a single-column FK is impossible. The buildable substitute
shipped is a SEPARATE nullable `workflow_run_id` FK (ON DELETE CASCADE), populated
via an existence-guarded subquery. The chat agent-host terminal path is DESCOPED
(DEC-6, flag-gated `ZIEE_CHAT_AGENT_CORE`, non-production, no rows when off).
