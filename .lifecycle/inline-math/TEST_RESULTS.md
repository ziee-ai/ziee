# TEST_RESULTS — inline math (Flavor B)

Every TEST-ID from TESTS.md, plus the required frontend gate lines. Full logs are the
background-task outputs referenced per section; nothing below is inferred from a partial
tail.

## Unit (`node --test` via the repo's `node-test-loader`) — 27/27 in the two math files

Command: `node --import src-app/ui/scripts/node-test-loader.mjs --test
src/components/common/normalizeMathDelimiters.test.ts
src/components/common/markdownPreprocess.test.ts` → `# pass 27 / # fail 0`.

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-20**: PASS

## E2E (Playwright, `--workers=1`, real backend + real browser)

Two runs; each of the five e2e TEST-IDs passed with a real assertion against the DOM.

- **TEST-15**: PASS — `renders \( … \) as inline math, not a display block` (both runs)
- **TEST-16**: PASS — `converts inline \( … \) in prose but skips regex alternation` (both runs)
- **TEST-17**: PASS — `leaves \[ … \] and \( … \) inside a code block literal` (run B)
- **TEST-18**: PASS — `an unpaired $ in the paragraph suppresses inline conversion` (run B)
- **TEST-19**: PASS — `renders \[ … \] display math (issue #177)`, `.katex-display` === 2 (run A)

**Run A** (full spec, 18 tests): 10 passed / 8 failed.
**Run B** (math subset after fixing the TEST-18 assertion, 6 tests): 4 passed / 2 failed.

**One real defect this surfaced, in the TEST-18 assertion (not the code).** Run A's
TEST-18 failed with `Expected "…the rate (k) is fixed." / Received "…the rate ( k ) is
fixed."`. The guard had behaved exactly right — `$5` literal, parens preserved, `.katex`
count 0 — but the expectation omitted the padding: the source is `\( k \)`, and markdown
drops the backslashes and keeps the spaces, so `( k )` IS "renders as it does today".
Assertion corrected; TEST-18 passes in run B. Worth stating plainly because a
`.katex === 0` check alone would have passed while the text assertion was wrong.

**Every other e2e failure across both runs is infrastructure, never a content assertion**,
and each is proven so by the failure mode:

| Spec | Failure | Why it is not this diff |
|---|---|---|
| mermaid / GFM table / Shiki highlighting | assertion timeout on the code-block locator | The spec's own `FIXME` at line 145 documents this exactly: vite cold-start + streamdown 2's dynamic import of the hashed Shiki chunk 504s when the run *starts* with a code-block render. Pre-existing and annotated in the file before this branch. |
| footnotes ×3, code-block-literal (run A) | `TimeoutError … waiting for [data-testid="app-setup-username-input"]` at `auth-helpers.ts:103` | The login/setup page never loaded — the per-test backend spawn degraded late in a 27-minute run. Content-independent; the same spec passed in run B. |
| `$$…$$` math, `\[ … \]` display (run B) | the identical `app-setup-username-input` timeout | Position-dependent, not content-dependent: both PASSED in run A as tests #4 and #5, and failed in run B only as tests #1 and #2. |

Taken together every spec in the file passed in at least one run except the
mermaid/table/Shiki trio, whose failure mode is the one the file already documents.

## Frontend gate lines

`gate:ui (ui): PASS` — `GALLERY_PORT=11731 npm run gate:ui -- --skip-visual`:

```
PASS  tsc          PASS  lint          PASS  runtime-health          PASS  visual (skipped)
runtime-health: 586/586 cells, 0 surface(s) with gating HIGH findings
--- per-surface runtime verdict: 182/182 PASS ---
✅ GATE PASSED — every UI DONE criterion met
```

(The default port 1420 was already bound by a *different* worktree's Vite; `GALLERY_PORT`
was overridden rather than killing another session's server.)

`npm run check (ui): FAIL` — **and it fails for a pre-existing reason this branch cannot
fix.** Recorded as FAIL rather than PASS because the command genuinely does not exit 0.

Sub-gate breakdown, all of it verified:

| Sub-gate | Result |
|---|---|
| `tsc` | PASS |
| `lint:guardrails`, `lint:colors`, `lint:settings-field`, `lint:adjacent-inline`, `lint:icon-action`, `lint:logical-direction`, `lint:tooltip-placement` | PASS (all seven) |
| `check:kit-manifest` | PASS |
| `check:design-spec` | PASS |
| **`check:testid-registry`** | **FAIL — stale** |
| **`check:gallery-coverage`** | **FAIL — stale** |
| **`check:state-matrix`** | **FAIL — stale** |

All three stale artifacts live in the **`sdk` submodule** (`sdk/packages/kit/src/testIds.generated.ts`
and siblings), whose pin `9e6d8c7` is behind the split-pane UI work already merged into the
ziee source at `origin/khoi`. Regenerating `testIds.generated.ts` produces a delta of purely
split-pane / conversation-picker / notification ids — nothing from this diff, which adds no
component, testid, or render state.

Proven pre-existing, not assumed:
1. `git stash`-ed all five changed files and re-ran `check:testid-registry` on the pristine
   base — identical failure.
2. The main checkout at `/home/khoi/ziee/ziee` has the *same* sdk pin with a clean working
   tree, so it fails identically. This is a property of `origin/khoi`, not of this worktree.

Fixing it would mean committing regenerated output to a **different repository** and bumping
the submodule pointer, dragging another workstream's in-flight ids into this PR. That is out
of scope and is left for the owner of that work. The regenerated files were reverted so this
branch leaves the submodule untouched.

## Other pre-existing failures encountered (see DRIFT-1.5)

- `MessageViewState.store.test.ts` / `SplitView.store.test.ts` fail with
  `ERR_MODULE_NOT_FOUND: @ziee/framework/src/store-kit` — the node test hook
  (`scripts/node-test-hooks.mjs:14-19`) extension-resolves only `@/`-prefixed specifiers,
  not bare-package subpaths. Proven pre-existing by the same stash-and-rerun method.
- The `sdk` submodule was uninitialized in a fresh worktree; `preflight.sh` checks
  `agent-kit` and `pgvector` but not `sdk`. Fixed locally via `git submodule update --init sdk`.
- `cargo build` from a directory outside `src-app` fails with `ZIEE_POSTGRES_VERSION not
  defined at compile time` — cargo discovers `.cargo/config.toml` from the working
  directory, so `--manifest-path` alone is not enough. Not a code issue; noted because it
  will bite anyone scripting the build.

## Deterministic phase-8 checks

- **A2** clean tree — all load-bearing files committed on the branch.
- **A3** no diff-added `#[ignore]` / `.skip` / `.only` — verified by grepping the added
  lines of `git diff origin/main...HEAD`.
- **A4** no cosmetic/always-true assertion — same grep; every new assertion compares a
  transform result or a DOM fact.
- **A5** TESTS.md did not shrink — TEST-20 was added; no TEST-ID removed.
- **A7** boot/runtime canary recorded above (`gate:ui (ui): PASS`).
- **A8 / A9 / A10** not engaged — no built-in MCP server and no permission introduced.
- **R2-5** no new `/api/` e2e route mock is added; the specs reuse the file's existing
  `seedAssistantWithText` / `mockChatStream` helpers.
