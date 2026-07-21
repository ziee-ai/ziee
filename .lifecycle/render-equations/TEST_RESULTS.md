# TEST_RESULTS — render-equations

Run against `fix/render-equations` at the final scoped state (display-only, chat +
file viewer). Full logs under the session scratchpad; key numbers inline.

## Unit — `npm run test:unit` (src-app/ui)

Command: `npm run test:unit`
Result: **456 tests, 446 pass, 10 fail.**

The 10 failures are **pre-existing and unrelated** — proven, not assumed: with this
branch's work stashed, a clean tree produces 423 tests / 413 pass / **the same 10
failures** (`auth/Auth.store`, `chat/core/stores/MessageViewState`,
`chat/core/stores/SplitView`, `chat/stores/ChatHistory`, `scheduler/runTimeline`,
`scheduler/stores/ScheduledTasks`, `voice/stores/VoiceModel*` ×4). All are
`*.store.test.ts`-style specs in modules this diff does not touch. Every test in
`src/components/common/` passes: **29/29**.

- **TEST-1**: PASS — display `\[ … \]` → `$$\n…\n$$`, multi-line body, doubly-escaped opener rejected
- **TEST-2**: PASS — inline `\( … \)` passthrough: sed, grep `\|` alternation, `grep -E '\(x\)'`, the `To escape use \( and \)` documentation sentence, genuine inline math, and an inline pair riding along inside a display body
- **TEST-3**: PASS — block positioning: mid-sentence, bullet list, ordered list, blockquote continuation prefixes
- **TEST-4**: PASS — guards: blank-line runaway, `$$` body line, 4-space AND tab indented code, link destination/title downgrade (plus the completed-link negative case), empty delimiters, table-row downgrade, nested `\[`, `$` in a downgraded inline body
- **TEST-5**: PASS — streaming partials unchanged, ReDoS body cap, pre-existing `$` math untouched, idempotence over every input in the file
- **TEST-6**: PASS — `markdownPreprocess`: code protection (fenced, `~~~`, inline span), the `$$ a[1] $$` + `[1]: url` math-span regression guard, byte-identical non-math corpus, original early return retained, sed-command-plus-reference-link case
- **TEST-11**: PASS — ReDoS body cap + nested-`\[` guard (repointed after the DEC-7 revert; see TESTS.md)

## Frontend gates

`gate:ui (ui): PASS` — tsc clean, lint clean, runtime-health **584/584 cells**,
**181/181 surfaces PASS**, **0 gating HIGH findings**, visual skipped
(`--skip-visual`).

> Run with `GALLERY_PORT=1481`. The default 1420 was occupied by a DIFFERENT
> worktree's dev server (`tmp/file-upload-size-cap-wt`). This matters beyond
> convenience: `gate-ui.mjs` *reuses* a server already on the port, so running on
> the default would have gated this branch against another branch's code. Killing
> that server was not an option — it belongs to concurrent work.

`npm run check (ui): FAIL — PRE-EXISTING, NOT CAUSED BY THIS DIFF.`

Five steps fail. **Proven pre-existing** by checking out the untouched base commit
`origin/khoi` (6ca93f123) and re-running: all five fail there identically.

| Step | Base `origin/khoi` | This branch |
|---|---|---|
| `check:testid-registry` | FAIL | FAIL |
| `check:gallery-coverage` | FAIL | FAIL |
| `check:state-matrix` | FAIL | FAIL |
| `check:overlay-registry` | FAIL | FAIL |
| `check:override-registry` | FAIL | FAIL |

Every other step passes on this branch: `tsc`, `lint:guardrails`, `lint:colors`,
`lint:settings-field`, `lint:adjacent-inline`, `lint:icon-action`,
`lint:logical-direction`, `lint:tooltip-placement`, `check:kit-manifest`,
`check:design-spec`, `check:gallery-crawl`, `gallery:check-fixtures`,
`check:gallery-seed-registry`.

The generated registries are behind the `khoi` source; the drift is other features'
(split-chat pane testids, notification testids). Regenerating was deliberately NOT
done: it would pull unrelated generated churn into this PR, and two of the five
outputs live in the **`sdk` submodule** (a separate repo), so "fixing" it means a
submodule bump unrelated to equation rendering. That is a separate change for
whoever owns that drift.

## E2E — BLOCKED (environment, not code)

`- **TEST-7**: BLOCKED`
`- **TEST-8**: BLOCKED`
`- **TEST-9**: BLOCKED`
`- **TEST-10**: BLOCKED`

Playwright's `global-setup.ts` starts a per-run PostgreSQL container. It cannot:

```
permission denied while trying to connect to the docker API at unix:///var/run/docker.sock
```

`id -nG` returns only `khoi` — the account is not in the `docker` group, and group
membership cannot be granted from inside this session (it needs a re-login).
The Rust server binary WAS built successfully, so this is the only blocker.

**These four are NOT claimed as passing.** To run them:

```bash
sudo usermod -aG docker "$USER"    # then log out and back in
cd src-app/ui && npx playwright test tests/e2e/chat/markdown-rendering.spec.ts --workers=1
```

What the e2e would add over the unit tests is browser-level proof; the underlying
rendering behavior IS verified by running — see below.

## Substitute verification for the blocked e2e (rule B7)

Every rendering claim was proven by RUNNING the real
`remark-parse → remark-gfm → remark-math → remark-rehype → rehype-katex` pipeline —
the exact plugin chain `@streamdown/math` wires — and asserting on the resulting
hast, rather than by reading code:

| Case | Result |
|---|---|
| Both literal #177 equations | `.katex-display` × 2 |
| `sed -e 's/\(foo\)/bar/'` | `.katex` × 0, text verbatim |
| `To escape use \( and \) in LaTeX.` | `.katex` × 0, text verbatim |
| `Pattern \(a\|b\) matched` | `.katex` × 0 |
| `Energy \( E = mc^2 \) is nice.` | `.katex` × 0 (accepted tradeoff) |
| `Energy $E = mc^2$ is nice.` | `.katex` × 1 (pre-existing path intact) |
| ` ```tex \[ x \] ``` ` | `.katex` × 0 |
| `- first \[ x_1 \]` | `.katex-display` × 1, `<li>` × 2 (list intact) |

Also verified by running: KaTeX emits the `.katex-mathml` `<math>` branch with
`<annotation encoding="application/x-tex">` and an `aria-hidden` visual branch (an
a11y improvement over raw LaTeX text); KaTeX `trust` is off so `\href`/`\htmlData`
inject nothing; and the ReDoS body cap turned the unmatched-opener scan from
quadratic (4× per doubling, 782 ms at 32k) to linear (2× per doubling, 121 ms).
