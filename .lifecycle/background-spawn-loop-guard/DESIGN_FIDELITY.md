# DESIGN_FIDELITY — background-spawn-loop-guard

- **INV-1** — fidelity: UPHELD — ITEM-1's guarded insert dedups on
  (conversation, job_kind, inputs_json) within the active/recent window and ITEM-3
  returns the existing run_id as a clear "already running/queued" result instead of
  a second run; race-safe via the per-conversation advisory lock.
- **INV-2** — fidelity: UPHELD — ITEM-1 counts non-terminal background runs per
  conversation against the cap (reused `fan_out_max_threads`) inside the same
  locked txn and refuses over-cap BEFORE the INSERT, so an over-cap spawn creates
  no row; ITEM-3 surfaces `BACKGROUND_SPAWN_CAP_EXCEEDED`.
- **INV-3** — fidelity: UPHELD — ITEM-4 verifies the resume injection carries no
  spawn directive, and the recent-completed dedup window (ITEM-1) refuses the
  identical re-spawn the re-engaged model attempts, so completion feedback yields
  no new run; the resumed turn's `spawn_background` also still requires approval.
