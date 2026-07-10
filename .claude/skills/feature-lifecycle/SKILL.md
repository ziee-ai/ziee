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
- **Restricted-user e2e** (`[negative-perm]` tag) — `- **TEST-7** (tier: e2e) [negative-perm] [covers: ITEM-3] file: \`.../foo.spec.ts\` — asserts: a user LACKING foo::use sees NO Foo UI (nav entry, page, composer, buttons all absent)`.
  REQUIRED whenever the diff introduces a user-facing permission
  (`X::use`/`X::read`/`X::manage` defined in a `modules/*/permissions.rs` or
  granted in a migration). This is the FRONTEND half of the authz gate (A10),
  paired with the backend deny test (A9). It must be `tier: e2e` — a 403/deny
  integration test does NOT satisfy it (that's A9).
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

**Preflight FIRST** — before writing a line of the plan, run the environment
gate so a setup problem surfaces now, not three phases deep into a red build:

```bash
bash .claude/lifecycle/preflight.sh --repo <worktree-root>
# exit 0 → env ready. non-zero → fix the printed problem (hub-seed, build-DB
# isolation, node_modules, pgvector submodule, stale Vite) before proceeding.
```

(This is the same gate on Linux, macOS, and Windows git-bash — it avoids
GNU-only tool flags and guards Unix-only tools.)

Write `.lifecycle/<feature>/PLAN.md` with three required sections:

- `## Items` — every unit of work as `- **ITEM-N**: <desc>`. IDs are the spine of
  the whole lifecycle: audits, tests, drift, and results all reference them.
- `## Files to touch` — the concrete files you expect to add/edit.
- `## Patterns to follow` — for each area, name the **closest existing module**
  to mirror (file structure, naming, idioms). This is a hard project rule
  ([[feedback_match_existing_patterns]]).

**P3 — conflict-surface scoping (BASE.md).** Also write a short
`.lifecycle/<feature>/BASE.md` recording what CURRENT main touches that this
branch will also touch: the highest existing migration number
(`ls src-app/server/migrations | tail -1`), any files/modules you expect to
edit that main is actively changing, and whether an `openapi.json` regen is
implied. This makes a migration-number or file collision visible at plan time
rather than as a build.rs failure after a long merge — and it is exactly what
the merge-gate re-checks against real main at merge time.

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

**Permission-gating rule (A10) — MANDATORY when your feature adds a
permission.** If the feature introduces a user-facing permission (a
`X::use`/`X::read`/`X::manage` defined in a `modules/*/permissions.rs` OR
granted in a migration), you MUST verify + test that **unpermitted users see
NOTHING** at every layer, and enumerate the **restricted-user e2e** that proves
the UI is ABSENT — not just 403-on-use. Walk all four gating layers from
`.claude/PERMISSION_GATING.md` — **slot → route → `<Can>` → `usePermission`** —
and assert, in a spec that logs in as a user LACKING the permission, that every
surface (sidebar/nav entry, route/page, composer, action buttons, menu items)
is absent. Tag it `[negative-perm]` at `tier: e2e`. The happy-path e2e (which
runs WITH the permission) can never catch an ungated surface; this spec is the
only thing that forces the negative case. The gate fails phase 3 if a permission
is introduced but no `(tier: e2e) [negative-perm]` spec is enumerated. This is
the FRONTEND half of the authz proof — paired with the backend deny test
(A9, phase 8).

> **Honest limit of the gate.** A10 enforces only that ONE restricted-user e2e
> *exists and passes* — it CANNOT verify that spec covers EVERY gated surface (a
> test could assert the nav entry is hidden yet miss an ungated composer). That
> is why the rule above is to walk ALL FOUR layers inside the spec; the gate is
> a floor, not a ceiling. Under-covering here is exactly how live2/live3/live4
> shipped ungated surfaces past a green 8/8 lifecycle.

Gate: `--phase 3` (fails loudly if any ITEM is unmapped, a UI diff has no e2e
test, or a new permission has no restricted-user `[negative-perm]` e2e).

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

**Configurable-settings rule (mandatory DEC).** Any operational tunable the
feature introduces — resource limits (memory/CPU/timeout/size caps), retention
periods, rate/quota limits, concurrency caps, feature enable/disable toggles,
model/provider selection, thresholds — MUST get an explicit DEC answering
**"fixed constant, or admin-configurable settings row?"** Default to
**admin-configurable** following the existing singleton-settings pattern
(`code_sandbox_settings` / `session_settings` / `memory_admin_settings`: a
settings table + migration with sane defaults, read-at-use with cache
invalidation, REST GET/PUT gated by a `<feature>::settings::{read,manage}`
permission, a sync entity, an admin settings card mirroring the closest
existing one, and bounds validation so an admin can't footgun the server).
Choose a fixed constant ONLY with an explicit rationale (e.g. a security
boundary that must not be operator-weakened) — and even then, structure it as a
`Limits`-style struct (not inline magic numbers) so it can be promoted to
configurable later without a rewrite. Never ship an operational tunable as a
bare hardcoded constant by omission. Enumerate the settings CRUD + gate + sync +
validation in TESTS.md when configurable.

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
   This includes any `[negative-perm]` **restricted-user** spec (A10): run it
   and record its TEST-ID as PASS — the gate requires the negative-permission
   e2e to pass, not just the happy-path one.

Write `TEST_RESULTS.md` with a `- **TEST-N**: PASS` line for **every** TEST-ID
from Phase 3 (plus the `npm run check (<ws>): PASS` line(s) above for a UI diff).
The gate fails if any phase-3 test is missing/not PASS, if a touched workspace has
no passing `npm run check` line, or if any enumerated `tier: e2e` spec is not
PASS. Never `#[ignore]` (or `.skip`) to go green — only genuine
platform-incompatibility is a legit skip ([[feedback_no_ignore_unless_platform]]).

**Phase 8 also enforces these deterministically** (from the diff + `TEST_RESULTS.md`),
so budget for them — they are not optional polish:

- **A2** clean working tree — every load-bearing file committed on the branch
  (no uncommitted change the gate can't see).
- **A3** no diff-added `#[ignore]`/`.skip`/`.only`; **A4** no cosmetic/always-true
  assertion (`assert!(true)`, `expect(x).toBe(x)`).
- **A5** TESTS.md may not shrink — a previously-enumerated TEST-ID cannot vanish.
- **A7** a UI diff must record a boot/runtime canary line
  (`gate:ui (<ws>): PASS`) — a green e2e can still ship a non-booting app or a
  root ErrorBoundary crash on an un-exercised path. **A6:** the gallery +
  `gate:ui` + `runtime-health` IS the browser-verify harness — "I can't verify
  in a browser" is not a valid gap.
- **A8** a new built-in MCP server must include BOTH `mcp.rs` edits
  (`auto_attach_builtin_ids` + `is_builtin_server_id`) — else it registers but
  the model never sees the tools.
- **A9** a new permission must have a BACKEND DENY-path test (403/forbidden),
  not only the allow path.
- **A10** a new user-facing permission (`X::use`/`X::read`/`X::manage` in a
  `modules/*/permissions.rs` or a migration grant) must ALSO have a
  **restricted-user e2e** — `(tier: e2e) [negative-perm]` — that logs in as a
  user LACKING the permission and asserts the feature UI is ABSENT, and it must
  be enumerated (phase 3) and PASS (phase 8). A9 proves the API refuses; A10
  proves the UI is hidden. Both are required — a 403 backend test alone leaves
  an ungated menu item / composer / nav entry invisible to the gate.
- **R2-5** every `/api/` e2e route-mock the diff adds must match a live route in
  `openapi.json` — a renamed route makes the mock a silent no-op that
  false-greens the spec.

**P4 — full-output capture.** Save the FULL test log as an artifact
(`| tee /data/pbya/ziee/tmp/lifecycle-logs/<feature>-{int,e2e}.log`), never just
an inline tail — the failing test's assertion/panic is in the body, and a
re-run to recover it wastes minutes ([[feedback_periodic_check_stuck]]).

Gate: `--phase 8`.

---

## Phase discipline (binding behavioral rules)

These are the human-judgment rules the deterministic gate cannot encode. They
are binding, not advisory — each traces to a specific failure that reached main
or wasted many sessions.

- **B1 — box load is NOT a deferral reason.** "The machine is busy, I'll do it
  later" is not a valid stop. Check actual CPU headroom (`%idle`/`%iowait` via
  `top`/`iostat`/Activity Monitor/`Get-Counter` on Windows), not the load
  average — a high load average with idle CPU is fine. "Blocked" requires a
  SPECIFIC error (a port bind failure, a docker daemon down, a compile error),
  never a resource metric.
- **B3 — never edit the SHARED test harness to route around YOUR feature's
  problem.** `tests/common/*`, the gallery cassette, `playwright.*.config`, the
  build DB helper are shared infrastructure. If your test needs them changed,
  that's a signal your feature is wrong or the change belongs in its own
  reviewed commit — not a silent workaround that breaks everyone else.
- **B4 — a warm build is NOT a clean build.** New SSE/content-block variants,
  proc-macro registrations, and codegen inputs can compile against a STALE
  expansion in an incremental tree yet fail from clean. Validate them with
  `cargo clean -p <crate> && cargo check` before believing them. (The
  authoritative catch is the merge-gate's C1; this is the cheap local mitigation.)
- **B5 — don't stop to ask permission mid-task.** Continue authorized work to
  completion; surface genuinely ambiguous PRODUCT decisions once, up front, in
  Phase 4 — not as a mid-implementation halt ([[feedback_autonomous_loop]]).
- **P1 — independently re-verify the load-bearing gate yourself.** Trust the
  ARTIFACT you re-ran, not a sub-agent's "it passes" self-report. Before
  declaring a phase done, re-run its gate in the actual worktree and read the
  output. This is the single biggest clean-vs-red differentiator.
- **P5 — native-verify a cfg-gated arm on ITS OWN platform.** A
  `#[cfg(target_os = "windows")]` / `#[cfg(target_os = "macos")]` block is
  invisible to a Linux `cargo check` — it is never compiled. Before pushing
  platform-specific code you MUST `cargo check` it on that OS (the Mac / Windows
  build hosts — [[project_crossplatform_build_test_hosts]]) and give it a
  cfg-gated unit test that runs there. A Linux-only green is not coverage of a
  Windows/macOS arm.
- **R2-3 — diff-review desktop `ui/` overrides against the server `ui/`
  equivalent (SECURITY).** `src-app/desktop/ui/` carries HAND-WRITTEN overrides
  (not just codegen). When you change logic in `src-app/ui/`, diff the desktop
  counterpart and confirm no security-relevant logic was dropped — a dropped
  `evaluatePermission` filter once reached desktop prod. Codegen'd files
  (`openapi.json`/`api-client/types.ts`) are regenerated for both by
  `just openapi-regen`; the hand-written surfaces are the risk.

**Cross-platform reality.** This infra runs on Linux, macOS, and Windows.
`lifecycle-check.mjs`, `merge-gate.mjs` are pure Node (portable); `preflight.sh`,
`selftest*.sh`, and the git hook run under bash — present on all three via
git-bash on Windows (git runs its hooks through that same bash). Keep any new
shell in this dir POSIX-portable (no GNU-only flags; guard Unix-only tools with
`command -v`).

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

## Merge hygiene (required)

`.lifecycle/<feature>/` artifacts live ON THE FEATURE BRANCH so the validator
and pre-push hook can gate it. They are process records, not product code —
**strip them when merging to main**: the merge driver (or the final commit
before merge) runs `git rm -r .lifecycle` so main never accumulates lifecycle
artifacts. The branch history preserves them for audit.

## Merge-gate (required before ANY push to main)

The per-branch `--all` gate CANNOT catch a collision with *current* main: the
pre-push hook EXEMPTS pushes to main by design, so a migration-number clash, a
stale branch, a dropped desktop regen, or a proc-macro variant that only fails
from a clean tree are all invisible to it. **Before merging/pushing to main, run
the merge-gate** — it stages the merge onto fresh `origin/main` and re-checks
against reality:

```bash
node .claude/lifecycle/merge-gate.mjs <feature-branch>
# C4 stale-branch · C2 migration-collision · staging-merge + P2 completeness ·
# C5 .lifecycle strip · C3 regen-parity (BOTH ui/ + desktop/ui/) · C1 clean build
# exit 0 → the merge onto current main is clean. non-zero → fix the reported gate.
```

Add `--keep-staging` to push the validated merge straight from the staging
worktree; `--skip-heavy` to run only the fast deterministic gates (C2/C4/P2/C5)
when iterating. This codifies — and replaces — the by-hand
staging-merge/`cargo clean`/regen-both-workspaces discipline.
