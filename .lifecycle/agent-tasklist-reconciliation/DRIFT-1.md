# DRIFT-1 — implementation vs plan (authored during phase 5)

- **DRIFT-1.1** — verdict: resolved (self-corrected during phase 5/7) — OpenAPI
  regen is NOT required; phase-2's original "NO" was right. Adding
  `TaskStatus::Abandoned` (ITEM-2) does force a total `From<TaskStatus>` for the
  wire enum `TaskListItemStatus`. I first added `Abandoned` to that wire enum too
  and concluded a regen was needed — WRONG on two counts: (1) `TaskListItemStatus`
  is DELIBERATELY a 3-value mirror of the FE `TaskItemStatus` union (its own
  doc-comment says so), so widening it fights its documented purpose; (2)
  `abandoned` NEVER travels this stream — the TaskListChanged SSE snapshot is the
  list loaded DURING the (non-terminal) turn, and reconciliation emits no SSE, so
  the `Abandoned` arm is unreachable here. Resolution: reverted the wire enum to 3
  values and mapped the unreachable `From` arm to `Pending` (a not-done item;
  never `Completed`). Net: `openapi.json` + `types.ts` are UNCHANGED (still 3
  values, consistent — the `emit_ts` parity test passes), NO regen, streaming.rs
  is the only extra file touched. (A clean `cargo clean -p ziee` + regen confirmed
  zero diff to the generated files, corroborating "no schema delta".)
- **DRIFT-1.2** — verdict: resolved — test seam. The plan said "extend
  `task_list_test.rs`". Reality: the reconcile fns + terminal writers live behind
  the private `modules` tree, so I exposed them via the sanctioned
  `ziee::test_internals` seam (its own comments endorse "fire the REAL repository
  fn instead of mirroring their SQL") and the tests drive the real functions.
  Added `lib.rs` to Files-to-touch. No plan conflict — strengthens the tests.
- **DRIFT-1.3** — verdict: none — ITEM-1/3/4/5/6 landed exactly as planned
  (reconcile primitive keyed by run_id; hooks at `mark_status` + `cancel_cas`;
  boot bulk sweep in `sweep_at_boot`; migration widens CHECK + adds the guarded
  `workflow_run_id` FK). Each item reconciled against INV-1..INV-4: no divergence.

**Unresolved drifts:** 0
