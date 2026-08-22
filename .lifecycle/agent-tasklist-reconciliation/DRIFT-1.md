# DRIFT-1 — implementation vs plan (authored during phase 5)

- **DRIFT-1.1** — verdict: impl-wins — OpenAPI regen IS required (phase-2 said
  NO). Adding `TaskStatus::Abandoned` (ITEM-2) forced a total `From<TaskStatus>`
  for the FE-facing wire enum `TaskListItemStatus` (chat streaming DTO,
  `#[derive(schemars::JsonSchema)]`). Rather than a dishonest map of `Abandoned`
  onto a live value, I added `Abandoned` to `TaskListItemStatus` too (a faithful
  1:1 mirror) — a public schema delta → `openapi.json` + `api-client/types.ts`
  regen in both binaries. FE impact is nil: the generated `TaskListItemStatus`
  has no FE importer (the FE uses its own hand-written `TaskItemStatus` union),
  so tsc stays green and no hand-written UI file changes → still NOT UI work.
  Amended: PLAN "Files to touch" (+ `chat/core/types/streaming.rs`, regen'd
  files, `lib.rs` test_internals), PLAN §"OpenAPI regen" (→ required), BASE.md.
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
