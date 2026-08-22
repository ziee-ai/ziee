# DESIGN_FIDELITY — one verdict per invariant

- **INV-1** — fidelity: UPHELD — ITEM-1 (primitive) + ITEM-3/4/5 (hooks at every
  terminal writer) guarantee no `pending`/`in_progress` row survives under a
  terminal run; the store stays the honest source of truth. Pinned by acceptance
  TEST-1.
- **INV-2** — fidelity: UPHELD — the reconcile UPDATE is scoped
  `WHERE status IN ('pending','in_progress')`, so `completed` rows are never
  touched, and the terminal value is `abandoned` (never `completed`), so no
  unfinished work is claimed done. Pinned by acceptance TEST-2.
- **INV-3** — fidelity: UPHELD — ITEM-4 covers user-cancel (`cancel_cas`) and
  ITEM-5 covers crash/restart-recovery (`sweep_at_boot`), the two non-happy paths
  the leak came from; ITEM-3 covers completed/failed/timeout. Pinned by
  acceptance TEST-3.
- **INV-4** — fidelity: UPHELD — ITEM-6 adds a real `workflow_run_id` FK with
  `ON DELETE CASCADE`, populated safely so a `workflow_runs` delete removes its
  task rows. Pinned by acceptance TEST-4.
