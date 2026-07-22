# DRIFT-1 — implementation vs plan

Audited the implemented diff against PLAN.md after finishing all items.

- **DRIFT-1.1** — verdict: resolved — ITEM-8 (`preprocessMarkdown`'s `[`-only early
  return silently making the whole feature a no-op) did not exist in the original plan; it
  was found during the phase-2 codebase audit and PLAN.md was amended before any code was
  written. Implementation matches the amended plan. Phases 1–3 were re-run green after the
  amendment.

- **DRIFT-1.2** — verdict: impl-wins — PLAN.md's *Files to touch* described the
  `markdownPreprocess.test.ts` change as only "fence/inline-code protection case", but the
  file contained TWO pre-existing tests that actively pin the OLD pass-through behavior and
  therefore had to be rewritten: `display math outside code is converted, inline is left
  alone` (asserts `inline \( x^2 \) here` is unchanged) and `the early return still
  short-circuits delimiter-free input` (whose comment asserts the ITEM-8 no-op is
  *correct*: "a string with `\(` and no `[` short-circuits, which is correct now that
  inline is passthrough"). That second one is notable — the bug ITEM-8 fixes was not just
  present, it was **pinned as intended behavior by a passing test**. PLAN.md amended to
  record the real scope. No new risk: both rewrites assert the Flavor-B behavior the plan
  already specifies.

- **DRIFT-1.3** — verdict: impl-wins — two comments asserting a now-false property were
  found in files already in scope and corrected under ITEM-7, which the plan had scoped to
  only two files. (a) `markdownPreprocess.test.ts:39-41` claimed `preprocessMarkdown` is
  used by "the skill drawer, workflow step output"; verified by grep that it has exactly
  THREE production call sites (both chat `TextContent`s + the file-viewer body) and
  neither skill nor workflow imports it. This matters beyond tidiness: DEC-10 keeps
  skill/workflow out of scope, and that comment implied this change would reach them. It
  does not. (b) The spec header at `markdown-rendering.spec.ts:40-42` still said inline
  `\( … \)` "is deliberately left alone". PLAN.md ITEM-7 amended to cover both.

- **DRIFT-1.4** — verdict: none — the plan listed the guards as 1–6 with the indented-code
  guard last; the implementation evaluates it BEFORE the paragraph-`$` scan. This is a
  pure ordering choice (cheapest-first: the paragraph scan is the only guard that reads
  beyond the match), and both orders produce identical output because every guard's action
  is the same `return whole`. Not a behavioral divergence.

- **DRIFT-1.5** — verdict: none — three PRE-EXISTING failures in this branch's base
  (`origin/khoi` @ `68af34059`) were discovered while running the gates. All were proven
  pre-existing by stashing the entire diff and re-running, and none is caused by or
  fixable within this feature; they are reported to the human rather than worked around.
  See TEST_RESULTS.md for the full record.
  1. The `sdk` submodule was uninitialized in a fresh worktree (preflight does not check
     it, unlike `agent-kit` and `pgvector`). Fixed locally with
     `git submodule update --init sdk` — an environment fix, not a code change.
  2. `MessageViewState.store.test.ts` and `SplitView.store.test.ts` fail with
     `ERR_MODULE_NOT_FOUND: @ziee/framework/src/store-kit`. The node test hook
     (`scripts/node-test-hooks.mjs:14-19`) extension-resolves only `@/`-prefixed
     specifiers, not bare-package subpaths.
  3. `npm run check` fails at three stale generated registries — `testIds.generated.ts`,
     `galleryCoverage.generated.ts`, `state-matrix` — all of which live in the **sdk
     submodule**, whose pin (`9e6d8c7`) is behind the split-pane UI work already present
     in the ziee source at `origin/khoi`. Regenerating them pulls in unrelated split-pane
     and notification testids and would require committing to a different repository, so
     it is deliberately NOT done here.

**Unresolved drifts:** 0
