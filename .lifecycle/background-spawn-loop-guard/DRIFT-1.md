# DRIFT-1 — background-spawn-loop-guard

Reconciliation of the shipped implementation against PLAN.md items + the design
invariants, authored live during phase 5.

- **DRIFT-1.1** — verdict: resolved — PLAN/INV-1 says "task-spec fingerprint"; the
  implementation dedups via JSONB equality (`inputs_json = $spec`) rather than a
  stored hash column. JSONB `=` compares canonical form (key-order/whitespace
  normalized), so it IS the canonical spec fingerprint — and it needs NO schema
  change (keeps LIGHT tier). Recorded as DEC-6's approach; no plan divergence, the
  invariant's promise is upheld exactly.
- **DRIFT-1.2** — verdict: none — ITEM-1 guarded insert lands in
  `workflow/repository.rs` as planned (`insert_background_run_guarded` +
  `GuardedBackgroundInsert`), with the advisory-lock TOCTOU guard from DEC-6.
- **DRIFT-1.3** — verdict: none — ITEM-2 `spawn_background_run` gained the
  `Option<BackgroundSpawnGuard>` param + `BackgroundSpawnResult` return as planned;
  all 3 call sites updated (2 in tools.rs, 1 in-module test).
- **DRIFT-1.4** — verdict: none — ITEM-3 `tools.rs` builds the guard from
  `fan_out_max_threads` (DEC-1) and maps the result (Duplicate→result, OverCap→
  error) as planned; applied to BOTH `subagent` and `sandbox_exec` spawners.
- **DRIFT-1.5** — verdict: none — ITEM-4 INV-3 is proven two ways as planned: the
  resume-message unit test (no spawn directive) + the recent-completed dedup
  integration test (completion re-inject does not re-spawn).
- **DRIFT-1.6** — verdict: resolved — an ENV drift (not a code/plan drift): the
  `sdk` submodule was not initialized in the worktree and could not be cloned (no
  network). Populated the sdk source at the branch's pinned commit
  (`4ab7530`) from the shared object store so the backend compiles; `cargo check
  -p ziee` and `cargo check --test integration_tests` both green. The sdk working
  tree is a submodule and will NOT be committed on this branch.

**Unresolved drifts:** 0
