# CROSS_REPO — workflow-kind-agent carry-along dependencies

This feature is authored on the app repo, but two GENERATED artifacts live in (or are entangled with)
the **`sdk` submodule** / **live1's SDK-extraction workstream**. They are NOT committed on this branch
(the sdk pointer is unchanged — `9e6d8c74` on both `feat/agent-core` and HEAD); they must be
regenerated + committed into the sdk + pointer-bumped at MERGE time, human-coordinated so they don't
conflict with the live SDK-extraction work.

## 1. Kit testid registry — REGENERATED + COMMITTED to the sdk (push the sdk commit at merge)
- **What:** this feature adds kit `data-testid`s (`wf-builder-*`, `wf-activity-*`, …). The canonical
  registry is `sdk/packages/kit/src/testIds.generated.ts` (a tracked, generated file in the sdk).
- **State on this branch:** regenerated AND committed into the sdk submodule (`git -C sdk` commit
  `550d087`), with the app's sdk pointer BUMPED to it on this branch. `check:testid-registry` PASSES.
  Before committing, `git -C sdk status` was CLEAN of any live1 work, so it was a safe normal commit
  (no clobber, no force). The regen also includes agent-core's pre-existing `agent-settings-*` ids.
- **THE ONE carry-along:** the sdk commit `550d087` is LOCAL (unpushed). At merge the human must
  **push it to the sdk remote** so the bumped pointer resolves. No push done here (guardrail).

## 2. Gallery-registry drift (kit→package move) — RECONCILED this phase (owned, not deferred)
- `check:gallery-coverage` / `check:state-matrix` / `check:overlay-registry` / `check:override-registry`
  previously failed on defunct `components/ui/{kit,shadcn}/*` surfaces + stale coverage entries from
  the kit→`@ziee/kit` package move. **FIXED this phase** (human override — own it, don't defer):
  regenerated the 3 generated registries, removed the stale entries from `coverage.ts` (54),
  `stateCoverage.ts` (~21 state keys) + `overlay-allowlist.json` (41), added this feature's 16
  builder/run surfaces + reconciled state keys, and generated the previously-uncommitted
  `src/core/overrides/OVERRIDE_MANIFEST.md`. All four checks PASS; `npm run check (ui)` is GREEN.
  All are APP-LOCAL files (committed on this branch) — no cross-repo, no live1-domain edits.

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

## 4. `npm run check (ui)` — now GREEN. The ONE remaining lifecycle-check artifact is Phase-6's stacked base.
`npm run check (ui)` is **PASS** (all 18 steps) after the §2 reconciliation + §1 sdk-testid commit.

The ONLY thing left is a lifecycle-check invocation artifact: lifecycle-check computes its diff base
from the `--base` flag ONLY (no env / app.config field — verified: `baseArg = opt('--base')`, else
default `origin/main`). Because `feat/workflow-kind-agent` is STACKED on **UNMERGED** `feat/agent-core`,
a BARE invocation (`origin/main...HEAD`) pulls the entire agent-core crate (100+ hunks — already
9/9-audited in agent-core's OWN lifecycle) into **Phase 6 (AUDIT_COVERAGE)** as if "new".

- With `--base feat/agent-core` (this feature's REAL diff), **Phase 6 is GREEN** and the full
  `lifecycle-check --all` is clean except… nothing — TEST_RESULTS records `npm run check (ui): PASS`.
- Against a bare `origin/main` invocation, Phase 6 fails on the agent-core hunks; this clears
  automatically when `feat/agent-core` merges to main.

**Every gate is green with the correct stacked base.** No feature-scope debt remains. The only
merge-time carry-along is pushing the sdk commit `550d087` (§1).
