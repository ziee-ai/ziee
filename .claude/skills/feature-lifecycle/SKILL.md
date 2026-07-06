---
name: feature-lifecycle
description: >
  Binding 8-phase state machine for ALL feature work in the ziee repo. Enforces
  plan → plan-audit → explicit test enumeration → up-front decisions → implement
  + drift-convergence loop → blind multi-angle audit with full diff coverage →
  fix/re-audit loop → gated test run. Every phase writes machine-checkable
  artifacts under .lifecycle/<feature>/ (committed on the branch); a deterministic
  validator gates each phase and a pre-push hook enforces the whole chain. Use
  whenever you start, resume, or review a feature/bugfix branch of nontrivial size.
---

# Feature Lifecycle

A phase state machine. **You may not enter phase N+1 until the validator passes
phase N:**

```bash
node .claude/lifecycle/lifecycle-check.mjs --phase <N> --repo <worktree-root>
# exit 0 → proceed.  non-zero → read the gap list, fix, re-run.
```

All artifacts live in `.lifecycle/<feature>/` **inside the feature worktree** and
are **committed on the branch** so they ride the PR and the pre-push hook can
read them. `<feature>` is a short kebab slug (e.g. `project-search`).

Work in a dedicated worktree off `origin/main`:

```bash
git worktree add -b feat/<slug> /data/pbya/ziee/tmp/<slug>-wt origin/main
mkdir -p /data/pbya/ziee/tmp/<slug>-wt/.lifecycle/<feature>
```

> The validator is **deterministic** — it enforces *structure and completeness*,
> not judgment. Passing it is necessary, not sufficient; the phases below carry
> the judgment. Never write filler to satisfy the parser.

---

## Artifact grammar (the validator parses these EXACT shapes)

Match these line formats precisely or the gate will not pass.

- **Plan item** — `- **ITEM-3**: <description>`
- **Audit verdict** — `- **ITEM-3** — verdict: PASS — <rationale>`
  (verdict ∈ `PASS | CONCERN | BLOCKED`; `BLOCKED` fails the gate)
- **Test** — `- **TEST-2** (tier: integration) [covers: ITEM-1, ITEM-3] file: \`path/to/test.rs\` — asserts: <what it proves>`
  (tier ∈ `unit | integration | e2e`). **UI work must enumerate ≥1 `tier: e2e`
  test** — the gate refuses an all-unit plan for a frontend-touching diff.
- **Decision** — `### DEC-1: <question>` then a `**Resolution:** <answer>` line
  and a `**Basis:** <convention|user|codebase>` line
- **Drift entry** — `- **DRIFT-1.2** — verdict: plan-wins — <text>`
  (verdict ∈ `plan-wins | impl-wins | none | resolved`); each DRIFT-*.md needs a
  `**Unresolved drifts:** <N>` summary line
- **Ledger row** (`LEDGER.jsonl`, one JSON/line) —
  `{"angle":"correctness","file":"src/...","line":42,"severity":"high","finding":"...","status":"confirmed"}`
- **Coverage row** (`AUDIT_COVERAGE.tsv`, tab-separated, header `file⇥start⇥end⇥angles`) —
  `src/foo.rs⇥120⇥145⇥correctness,security,perf`
- **Fix round** (`FIX_ROUND-1.md`) — a `**New confirmed findings:** <N>` line
- **Test result** (`TEST_RESULTS.md`) — `- **TEST-2**: PASS`
- **Frontend gate line** (`TEST_RESULTS.md`, REQUIRED once the diff touches a UI
  workspace) — `npm run check (ui): PASS` — one line per touched workspace; the
  label is `ui` (→ `src-app/ui`) or `desktop/ui` (→ `src-app/desktop/ui`)

---

## Phase 1 — PLAN.md

Write `.lifecycle/<feature>/PLAN.md` with three required sections:

- `## Items` — every unit of work as `- **ITEM-N**: <desc>`. IDs are the spine of
  the whole lifecycle: audits, tests, drift, and results all reference them.
- `## Files to touch` — the concrete files you expect to add/edit.
- `## Patterns to follow` — for each area, name the **closest existing module**
  to mirror (file structure, naming, idioms). This is a hard project rule
  ([[feedback_match_existing_patterns]]).

Gate: `--phase 1`.

## Phase 2 — PLAN_AUDIT.md

Audit the plan *against the codebase* before writing code. Required dimension
sections (`## Breakage risk`, `## Pattern conformance`, `## Migration collisions`,
`## OpenAPI regen`) plus a per-item verdict line for **every** ITEM:

```
- **ITEM-1** — verdict: PASS — mirrors project/repository.rs; no new migration
- **ITEM-2** — verdict: CONCERN — needs `just openapi-regen` (new response field)
```

Check: does any item break an existing caller? Does it conform to the reference
module? Do migration numbers collide with `ls migrations/`? Does a type change
require `just openapi-regen` in BOTH ui and desktop? Any `BLOCKED` verdict fails
the gate — resolve it (amend the plan) first.

Gate: `--phase 2`.

## Phase 3 — TESTS.md

Enumerate **every** test up front. The gate enforces a bipartite mapping:
**every ITEM is covered by ≥1 TEST**, and every TEST names a valid ITEM, a tier,
a target file, and what it asserts. Be comprehensive — mirror the codebase's
existing tier pattern (unit `#[cfg(test)]` / integration `tests/<module>/` / e2e
`ui/tests/e2e/`) ([[feedback_comprehensive_tests]]). No cosmetic tests — mock
only the external boundary ([[feedback_no_cosmetic_tests]]).

Enumerate tests per tier for each ITEM. A backend item usually gets unit +
integration; **a user-visible UI item MUST also get an `e2e` spec** for the flow:

```
- **TEST-4** (tier: unit)        [covers: ITEM-3] file: `src-app/ui/src/modules/foo/Foo.store.ts` — asserts: store reducer maps the response
- **TEST-5** (tier: e2e)         [covers: ITEM-3] file: `src-app/ui/tests/e2e/foo/foo.spec.ts` — asserts: user opens Foo, submits, sees the result
```

The gate computes touched areas from the diff (or, before any code exists,
PLAN.md's *Files to touch*). If a **frontend** path (`src-app/ui/**` or
`src-app/desktop/ui/**`, ignoring the mechanically-generated
`openapi.json`/`api-client/types.ts`) is touched and **no `tier: e2e` test is
enumerated, the gate fails** — an all-unit plan for UI work is refused. Budget
the e2e specs here; phase 8 runs them.

Gate: `--phase 3` (fails loudly if any ITEM is unmapped, or a UI diff has no e2e test).

## Phase 4 — DECISIONS.md

Identify **every** human/product input the whole implementation will need, UP
FRONT, and resolve each — so implementation then runs nonstop. Prefer resolving
by existing convention and record the rationale; batch anything genuinely
ambiguous into ONE `AskUserQuestion` at plan time. **Zero** `TBD`/`TODO`/`ASK`/
`???` markers may remain (the gate greps for them).

```
### DEC-1: How is the search matched — prefix or substring?
**Resolution:** case-insensitive substring (ILIKE '%q%')
**Basis:** convention — matches conversations title filter in chat/repository.rs
```

Gate: `--phase 4`.

## Phase 5 — Implement + drift loop

Implement all items (only `cargo check` / `tsc` mid-flight; don't run the full
suites yet — [[feedback_finish_all_before_testing]]). Then audit
**implementation vs plan** and write `DRIFT-1.md`. For each divergence:

- `plan-wins` → the impl is wrong; re-implement that part to match the plan.
- `impl-wins` → the plan was wrong; **amend PLAN.md** (and re-run `--phase 1..3`)
  with the rationale captured in the drift entry.
- `none`/`resolved` → reconciled.

End each round with `**Unresolved drifts:** <N>`. Repeat (`DRIFT-2.md`, …) until a
round records **0** unresolved drifts.

Gate: `--phase 5` (checks the final round is 0).

## Phase 6 — Blind multi-angle audit

Spawn **fresh/blind** subagents (diff-only context: `git diff main...HEAD`) — do
NOT hand them your reasoning. Use ≥10 angles from the proven roster:

`correctness · security · error-handling · concurrency · perms/authz ·
api-contract · state-management · a11y · patterns-conformance · tests-quality ·
perf · i18n/copy`

Each angle appends findings to `LEDGER.jsonl`. **Coverage law:** every hunk of
`git diff main...HEAD --unified=0` must appear in `AUDIT_COVERAGE.tsv` as reviewed
by **≥3 distinct angles**. The validator parses the real diff and reconciles it
against the TSV — any uncovered hunk fails the gate. (Forks that share the
parent's cached context are the cheap way to fan out — [[feedback_fork_cache_review]].)

> The coverage law excludes the lifecycle artifacts and **mechanically-generated
> files** (`**/openapi.json`, `**/api-client/types.ts`) — those are derived
> deterministically from reviewed source by a golden-tested generator, so review
> the *source* hunks, not the generated output. (The same exclusion is why a
> backend feature that merely regenerates the client is **not** treated as UI
> work by the phase 3 / phase 8 frontend gates.) A regen may produce a large
> positional (key-order) diff in `openapi.json` with a tiny content delta; verify
> the content delta with `comm` on sorted files and record it as a drift entry.

Gate: `--phase 6`.

## Phase 7 — Fix / re-audit loop

Merge the ledger → fix every confirmed finding → **re-run a full blind round**.
Record each round in `FIX_ROUND-<n>.md` ending with `**New confirmed findings:**
<N>`. Repeat until a full blind round yields **0** new confirmed findings. (Reject
false positives explicitly in the ledger; a dismissed finding is not a fix.)

Gate: `--phase 7` (checks the final round is 0).

## Phase 8 — Gated test run (conditional on the touched areas)

ONLY NOW run tests, scoped to what you built ([[feedback_test_scope]]). **Which
gates apply is computed from `git diff main...HEAD`** (generated
`openapi.json`/`api-client/types.ts` excluded, so they never make a backend diff
look like UI work). A diff that touches both back- and front-end runs BOTH
chains.

**If the diff touches the backend** (`src-app/server/**`, `src-app/desktop/tauri/**`):

```bash
# integration (source the env file first — real LLM keys live there)
source src-app/server/tests/.env.test
cargo test --test integration_tests <module>:: -- --test-threads=1 \
  2>&1 | tee /data/pbya/ziee/tmp/lifecycle-logs/<feature>-int.log
```

**If the diff touches a frontend workspace** (`src-app/ui/**` and/or
`src-app/desktop/ui/**`), ALL of the following are required, per touched
workspace:

1. **`npm run check` — the one gate command, run in EACH touched workspace.** It
   chains the whole static frontend contract: `tsc` + the biome guardrails +
   `lint:colors` + `lint:settings-field` + `check:kit-manifest` +
   `check:testid-registry` + `check:design-spec` + `check:gallery-coverage` +
   `check:state-matrix`. Don't run these individually — run the one command and
   record its result:
   ```bash
   cd src-app/ui && npm run check          # and src-app/desktop/ui if touched
   ```
   Write a `npm run check (ui): PASS` line (and `npm run check (desktop/ui): PASS`)
   in `TEST_RESULTS.md` — the gate requires one per touched workspace.
2. **New conditional render states need gallery coverage.** Any new
   loading/empty/error/variant state your diff introduces must have a gallery
   entry (or an explicit allowlist reason). This is enforced by the
   `check:state-matrix` gate *inside* `npm run check` above — budget for it: if
   you added a state, add its gallery cell or the gate (and thus phase 8) fails.
3. **UI evaluator gate** (per CLAUDE.md's "UI Build Gate"): zero console
   errors / uncaught exceptions / failed requests / AA-contrast failures on the
   touched gallery surfaces, and the visual-regression baseline matches. Run the
   gallery runtime + gate scripts:
   ```bash
   cd src-app/ui && npm run gate:ui                     # runtime-health + Layer A/axe + tsc/lint
   VISUAL_SNAPSHOTS=1 npm run gate:ui                   # + Layer B pixel regression vs baseline
   ```
4. **e2e specs for the user-visible flows** you enumerated as `tier: e2e` in
   phase 3 (TESTS.md) — run them and record each TEST-ID:
   ```bash
   cd src-app/ui && npx playwright test tests/e2e/<file> --workers=1
   ```

Write `TEST_RESULTS.md` with a `- **TEST-N**: PASS` line for **every** TEST-ID
from Phase 3 (plus the `npm run check (<ws>): PASS` line(s) above for a UI diff).
The gate fails if any phase-3 test is missing/not PASS, if a touched workspace has
no passing `npm run check` line, or if any enumerated `tier: e2e` spec is not
PASS. Never `#[ignore]` (or `.skip`) to go green — only genuine
platform-incompatibility is a legit skip ([[feedback_no_ignore_unless_platform]]).

Gate: `--phase 8`.

---

## Finishing

```bash
node .claude/lifecycle/lifecycle-check.mjs --all --repo <worktree-root>   # must be all-green
git add -A && git commit && git push        # pre-push hook re-runs --all
```

The pre-push hook blocks the push unless `--all` is green. To push a
deliberately-incomplete WIP branch, `git push --no-verify` (and say so).

## Notes

- Keep commit messages clean — no Claude trailers ([[feedback_no_claude_trailers]]).
- If a decision genuinely can't be resolved by convention, surface it EARLY
  rather than guessing ([[feedback_research_landscape_before_plan]]).
- The lifecycle artifacts are committed; the skill/validator/hook are machine-local.
