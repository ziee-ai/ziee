# TESTS — render-equations

Every PLAN ITEM mapped to at least one enumerated test. ITEM-9 is `[DESCOPED]`
with an approved disposition in DECISIONS.md (DEC-6) and is therefore exempt.

Tiers: `unit` = `node:test` via `npm run test:unit`; `e2e` = Playwright.
No new permission is introduced, so no `[negative-perm]` spec is required (A10 N/A);
no backend path is touched, so A9 is N/A.

## Unit — the normalizer

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: display `\[ … \]` becomes block math with the content on its own line — `'\\[ x^2 + y^2 = z^2 \\]'` → `'$$\nx^2 + y^2 = z^2\n$$'`; a multi-line body (`\frac{a}{b} \\` + `c = d`) keeps its internal newlines; and a doubly-escaped `'\\\\[ not math \\\\]'` is left unchanged by the opener lookbehind.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: inline `\( … \)` passes through BYTE-IDENTICAL, so the POSIX-BRE collision can never corrupt prose — `sed -e 's/\(foo\)/bar/'`, `Pattern \(a\|b\) matched`, `grep -E '\(x\)'`, and the documentation sentence `To escape use \( and \) in LaTeX.` are all unchanged; genuine inline math `\( E = mc^2 \)` is likewise unchanged (the accepted tradeoff); and an inline pair INSIDE a display block rides along in the converted body.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: block positioning and container continuation prefixes — mid-sentence `'Given \\[ E = mc^2 \\] we conclude.'` → `'Given \n$$\nE = mc^2\n$$\n we conclude.'`; bullet `'- first \\[ x_1 \\]\n- second'` → 2-space-prefixed fences; ordered `'1. step \\[ \\frac{a}{b} \\]'` → 3-space prefix; blockquote `'> quote \\[ x \\]\n> more'` → every emitted line carries `> `.
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: each guard returns the input unchanged or safely downgraded — blank line inside the delimiters is not converted; a body line that is exactly `$$` is not converted; a 4-space-indented `'    \\[ x \\]'` (indented code) is not converted; empty `'\\[\\]'` / `'\\(  \\)'` are not converted; a table row `'| a | \\[ x^2 \\] |'` downgrades to inline `'| a | $x^2$ |'`; inline whose inner contains `$` (`'\\( a $ b \\)'`) is not converted while display with `\$5` IS converted.
- **TEST-5** (tier: unit) [covers: ITEM-1, ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: streaming safety and idempotence — an unclosed `'streaming \\[ \\frac{k}'` and `'partial \\( x'` pass through byte-identical; `'\\[ a \\] then \\[ b'` converts the complete pair and leaves the trailing partial untouched; pre-existing `'keep $x$ and $$y$$'` is untouched; and `f(f(x)) === f(x)` holds for every input used across TEST-1..5.

## Unit — the shared preprocessor (highest blast radius)

- **TEST-6** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/components/common/markdownPreprocess.test.ts` — asserts: (a) code protection — `\[ x \]` inside a fenced block and inside an inline code span are both left literal; (b) the math-span regression guard — `'$$ a[1] $$\n\n[1]: http://x'` leaves the math span untouched (proving the shortcut-reference regex no longer reaches inside math); (c) **byte-identical on non-math input** — a corpus of reference-link forms (full `[t][id]`, collapsed `[t][]`, shortcut `[t]`, a `[id]: url "title"` definition, a footnote `[^1]`, an array index, a fenced block, an inline span) produces exactly the same output as the pre-change implementation, captured as literal expected strings; (d) the ORIGINAL early return is retained unchanged (`\[` contains `[`, so no widening is needed once inline is passthrough) and still short-circuits bracket-free input; (e) a `sed -e 's/\(foo\)/bar/'` command alongside a resolvable `[docs]` reference proves the reference pass still fires while the inline delimiters survive.

## E2E — real rendering in the browser (B7: verification means running it)

- **TEST-7** (tier: e2e) [covers: ITEM-1, ITEM-6] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: an assistant message containing the ACTUAL issue-#177 equations (`\[ \frac{d^2C(x)}{dx^2} - \frac{k}{D}C(x) = 0 \]` and `\[ C(x) = C_0 \, e^{-x/\lambda}, \quad \lambda = \sqrt{D/k} \]`) renders exactly 2 `.katex-display` blocks in the chat bubble, and the raw LaTeX source text is NOT present. This is the literal reproduction of the reported bug (rule B9).
- **TEST-8** (tier: e2e) [covers: ITEM-4, ITEM-5] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: a `\[ x \]` inside a fenced code block renders zero `.katex` elements and the literal `\[ x \]` text is still visible in the code block — the code-protection guarantee proven in the real renderer, not just the pure function.
- **TEST-9** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: the inverted former negative test — `$$x^2 + y^2 = z^2$$` in an assistant message now renders `.katex` (count > 0), pinning that the KaTeX plugin stays wired and that the retired `[[no-katex-remark-rehype]]` directive does not creep back.
- **TEST-10** (tier: e2e) [covers: ITEM-1, ITEM-4] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: **repointed after the DEC-12 descope** (was the skill-drawer spec, which is now out of scope per DEC-7). An assistant message containing `sed -e 's/\(foo\)/bar/'` and the sentence `To escape use \( and \) in LaTeX.` renders BOTH verbatim, with zero `.katex` elements — the inline-passthrough decision proven at the rendered-DOM level, not just in the pure function.
- **TEST-11** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: **repointed after the DEC-7 revert** (was the workflow-step spec). The two guards added by the blind audit — a display body longer than the 2000-char ReDoS cap does not convert, and a nested `\[` inside a display body leaves the whole construct literal rather than emitting a block plus a dangling `\]`.

## Regression suites that must stay green (blast-radius guard, ITEM-5/6)

These are not new tests; they are pre-existing suites the change must not break, and
they are recorded here because ITEM-5/6 alter code they exercise. Their results are
recorded in TEST_RESULTS.md alongside the enumerated tests.

- The FULL `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` suite (not only the
  math cases) — it covers reference links, images, tables, code fences and mermaid
  through the chat renderer ITEM-6 changes.
- `npm run test:unit` in `src-app/ui` in its entirety — includes
  `citationTokenize.test.ts`, `footnoteScope.test.ts`,
  `modules/file/utils/markdownRoundtrip.test.ts`.
- `npm run check (ui): PASS` and `gate:ui (ui): PASS` — the static + runtime-health
  gates required for any UI diff.
