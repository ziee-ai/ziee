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
- **TEST-21**: PASS
- **TEST-22**: PASS
- **TEST-23**: PASS
- **TEST-24**: PASS
- **TEST-25**: PASS
- **TEST-26**: PASS
- **TEST-27**: PASS

## E2E (Playwright, `--workers=1`, real backend + real browser)

Two runs; each of the five e2e TEST-IDs passed with a real assertion against the DOM.

- **TEST-15**: PASS — `renders \( … \) as inline math, not a display block`
- **TEST-16**: PASS — `converts inline \( … \) in prose but skips regex alternation`
- **TEST-17**: PASS — `leaves \[ … \] and \( … \) inside a code block literal`
- **TEST-18**: PASS — `an unpaired $ in the paragraph suppresses inline conversion`
- **TEST-19**: PASS — `renders \[ … \] display math (issue #177)`, `.katex-display` === 2

**Final run** (full spec, 18 tests, after the ITEM-10 guard change): **17 passed / 1
failed** in 22.0m. The single failure is `renders fenced code with Shiki highlighting`.

Earlier runs were much noisier (10/18, then 4/6) and I first wrote those failures off as
"the per-test backend spawn degraded late in a long run". **That was wrong**, and the
correction matters more than the original claim: the Playwright error-context showed the
browser had landed on **MinIO's console page**, not the app. The e2e port-manager
allocates worker 0 `vite 9000 / backend 9100` (`tests/fixtures/port-manager.ts:303`), and
this host runs a MinIO container publishing `9000-9001` — so the preview server's port was
already taken and every affected spec timed out waiting for a login field that was never
going to appear. Re-running with the harness's own escape hatch:

```bash
ZIEE_E2E_BASE_VITE_PORT=9600 ZIEE_E2E_BASE_BACKEND_PORT=9700 \
  npx playwright test tests/e2e/chat/markdown-rendering.spec.ts --workers=1
```

took the suite from 10/18 to 17/18 — mermaid, GFM table, both display-math specs and all
three footnote specs went green, none of which had anything to do with this diff. Anyone
running e2e on this box needs those two vars.

**One real defect this surfaced, in the TEST-18 assertion (not the code).** Run A's
TEST-18 failed with `Expected "…the rate (k) is fixed." / Received "…the rate ( k ) is
fixed."`. The guard had behaved exactly right — `$5` literal, parens preserved, `.katex`
count 0 — but the expectation omitted the padding: the source is `\( k \)`, and markdown
drops the backslashes and keeps the spaces, so `( k )` IS "renders as it does today".
Assertion corrected; TEST-18 passes in run B. Worth stating plainly because a
`.katex === 0` check alone would have passed while the text assertion was wrong.

**The one remaining failure is not this diff.** `renders fenced code with Shiki
highlighting` fails on:

```
Locator: …[data-streamdown="code-block"][data-language="rust"] … pre span[style*="color"]
Expected: visible — element(s) not found
```

i.e. the code block renders but Shiki's per-token `style="color:…"` spans never arrive.
That is exactly the condition the spec's own `FIXME` at line 145 documents — vite
cold-start racing streamdown 2's dynamic import of its hashed Shiki chunk
(`[[streamdown-v2-unbundled-plugins]]`) — it is a syntax-highlighting concern with no
connection to math delimiters, and this diff touches no code-block path. It reproduces on
the branch base.

## Live container verification (real model, real browser)

Not a lifecycle-required tier — run because a string transform passing unit tests can
still be wrong about what a real model emits and what the real renderer does with it.

**Stack** (`ziee-inline-check`, isolated from the host's existing deployment on :8080):
postgres + the `ziee-web:local` API + an nginx SPA image built from THIS branch via
`deploy/runtime/web.Dockerfile`, seeded with `deploy/seed/*.sql` (admin, the Free Models
provider, GPT-OSS 120B, BioGnosia MCP), provider repointed at the host's LiteLLM.
Published on :8098.

**Round 1 — real model output.** Asked GPT-OSS 120B for the mass-energy equivalence. It
emitted, unprompted, exactly the shape Flavor B exists for — bare single symbols:

```
\[
E = m c^{2},
\]

and in this formula \(E\) denotes the energy, \(m\) the rest mass, and \(c\) the …
```

Rendered: 4 `.katex` = 1 display + 3 inline; TeX annotations `E = m c^{2},`, `E`, `m`, `c`;
no `\(` anywhere in the DOM. Note `\(E\)` / `\(m\)` / `\(c\)` carry no `=`, no `^`, no
LaTeX command — a content-gated Flavor A would have missed all three.

**Round 1 also exposed a real defect** (→ ITEM-10, DRIFT-3). The user-message bubble, one
run-on paragraph containing `\[ … \]`, rendered its `\( c \)` as literal `( c )`: the
display pass had emitted `$$` into that same paragraph and the coarse dollar-guard
suppressed every inline span there. Found by LOOKING at the render, not by a test.

**Round 2 — after tightening the guard to pair `$` runs by length.** Deterministic probe
through the same pipeline (user bubble, single paragraph):

```
Check: the energy is \[ E = mc^2 \] where \( m \) is the rest mass and \( c \) is …
```

→ 3 `.katex` = **1 display + 2 inline**, TeX `["E = mc^2","m","c"]` — the display block and
BOTH inline spans, from one paragraph. The assistant's reply in the same run rendered 7
`.katex` (2 display + 5 inline), including inline math inside bullet-list items and
`3.00\times10^{8}\,\text{m/s}`.

**Round 3 — conversation titles (ITEM-11).** The stored title for the round-2 conversation
is literally `Check: the energy is \[ E = mc^2 \] where \( m \) ...`, so it is a real
fixture rather than a synthetic one. After the fix, read straight out of the DOM:

```
SIDEBAR rows : ["Check: the energy is E = mc^2 where m ...", …]
HEADER       : ["Check: the energy is E = mc^2 where m ...", …]
stray delimiters anywhere on the page: none
```

## Full frontend unit suite

`npm run test:unit` (whole workspace, not just the math files): **464 passed / 10 failed**.

All 10 failures are `ERR_MODULE_NOT_FOUND` — **zero assertion failures**, and none in a
file this branch touches. Eight are tests importing a module that no longer exists
(`Auth.store`, `ChatHistory.store`, `runTimeline`, `ScheduledTasks.store`, four
`VoiceModel*.store`) — left behind by other in-flight work that moved those modules; two
are the `@ziee/framework/src/store-kit` resolution gap recorded in DRIFT-1.5. Verified
pre-existing by the stash-and-rerun method.

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
