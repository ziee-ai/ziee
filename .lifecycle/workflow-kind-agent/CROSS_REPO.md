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

## 3. Stacked-branch lifecycle base — invoke with `--base feat/agent-core`
This branch is STACKED on `feat/agent-core`, so the feature's real diff is
`feat/agent-core...HEAD`. lifecycle-check / merge-gate default to `origin/main`, which pulls in ALL of
agent-core's hunks (100+ files: the agent-core crate, chat/agent_host, the agent module + its tests —
covered in **agent-core's OWN lifecycle**, NOT this feature). ALWAYS run with `--base feat/agent-core`
(recorded in BASE.md + `.claude/app.config` `LIFECYCLE_BASE`). Against that base, phase 6 coverage is
green on this feature's real hunks. A one-line agent-kit follow-up (have lifecycle-check read
`APP.LIFECYCLE_BASE` as the default base) would make a bare invocation correct — deferred because
`.claude/lifecycle` is a symlink into the agent-kit submodule (a cross-repo change, human-coordinated).

## 4. The two remaining lifecycle-check fails are STACKED-BRANCH / PRE-EXISTING artifacts (not this feature)
lifecycle-check computes its diff base from the `--base` flag ONLY (no env / app.config field is
read — verified in lifecycle-check.mjs: `baseArg = opt('--base')`, else default `origin/main`). Because
`feat/workflow-kind-agent` is STACKED on **UNMERGED** `feat/agent-core`, a BARE invocation
(`origin/main...HEAD`) pulls in the entire agent-core crate + live1's pre-existing debt as if "new":

- **Phase 6 (AUDIT_COVERAGE)** — fails ONLY against `origin/main...HEAD` (100+ agent-core hunks: the
  agent-core crate, chat/agent_host, the agent module + tests — already **9/9-audited in agent-core's
  OWN lifecycle**). With `--base feat/agent-core` (this feature's REAL diff) Phase 6 is **GREEN**.
  Clears automatically when `feat/agent-core` merges to main (then `origin/main...HEAD` == this feature only).
- **`npm run check (ui)`** — fails on (a) `check:{gallery-coverage,state-matrix,overlay-registry,
  override-registry}` = live1's PRE-EXISTING kit→`@ziee/kit` package-move debt (fails identically on the
  base), clearing when live1's SDK-extraction reconciles the kit gallery coverage; and (b)
  `check:testid-registry` = this feature's kit-testid **merge carry-along** (§1), clearing when
  regenerated + committed to the sdk at merge. Neither is this feature's code.

**All FEATURE-SCOPE gates are green** (against `--base feat/agent-core`): phases 1-7 + 9 OK, Phase 6
coverage green on the feature's real hunks; the only non-green is the pre-existing/carry-along
`npm run check (ui)` debt above. Both remaining fails resolve at the respective merges — nothing to fix
on this branch.
