# CROSS_REPO — workflow-kind-agent carry-along dependencies

This feature is authored on the app repo, but two GENERATED artifacts live in (or are entangled with)
the **`sdk` submodule** / **live1's SDK-extraction workstream**. They are NOT committed on this branch
(the sdk pointer is unchanged — `9e6d8c74` on both `feat/agent-core` and HEAD); they must be
regenerated + committed into the sdk + pointer-bumped at MERGE time, human-coordinated so they don't
conflict with the live SDK-extraction work.

## 1. Kit testid registry — MUST regenerate + commit to sdk at merge
- **What:** this feature adds kit `data-testid`s (`wf-builder-*`, `wf-activity-*`, …). The canonical
  registry is `sdk/packages/kit/src/testIds.generated.ts` (a tracked, generated file in the sdk).
- **State on this branch:** NOT regenerated in the committed tree — so `check:testid-registry` FAILS
  on a fresh `npm run check` (my kit ids aren't in the committed sdk registry yet). The regen IS green
  when run on-disk (verified this phase); I reverted it so the working tree stays clean for the A2
  gate (a `LIFECYCLE_CLEAN_TREE_IGNORE` scoped to the submodule couldn't reliably match — the
  lifecycle git wrapper `.trim()`s the porcelain and mangles the first status line's path). The sdk
  POINTER is unchanged (9e6d8c74).
- **At merge (human):** `cd src-app/ui && npm run gen:testid-registry`, commit
  `testIds.generated.ts` inside the sdk submodule, and bump the app's sdk pointer to that commit
  (push the sdk commit). NOTE the regen also includes agent-core's pre-existing `agent-settings-*`
  ids (the base's own un-committed registry debt) — expected; they converge when agent-core merges.
- **Same pattern** the workbench-shell feature flagged for kit-testid carry-along.

## 2. Gallery-registry drift (kit→package move) — LEFT for live1 (base debt, not this feature)
- `check:gallery-coverage` / `check:state-matrix` / `check:overlay-registry` / `check:override-registry`
  fail on ~91 defunct `components/ui/kit/*` surfaces in the committed generated gallery files + 73
  stale entries in `coverage.ts`/`stateCoverage.ts` — pre-existing from before the kit moved into
  `@ziee/kit`. Fails identically on the pristine base. **Not fixed here** (whole-app cleanup inside
  live1's SDK-extraction domain). This feature's OWN gallery surfaces are verified present + covered.
  See TEST_RESULTS.md → PRE-EXISTING BASE DEBT.

## Not cross-repo (committed normally on this branch)
- OpenAPI regen (both binaries: `ui/` + `desktop/ui/` openapi.json + api-client/types.ts) — committed.
- App-level source, tests, and the `agent_dispatch.rs`/`dev.rs`/`repository.rs` backend — committed.
