# TESTS — render-equations

Every PLAN ITEM mapped to at least one enumerated test. ITEM-9 is `[DESCOPED]`
with an approved disposition in DECISIONS.md (DEC-6) and is therefore exempt.

Tiers: `unit` = `node:test` via `npm run test:unit`; `e2e` = Playwright.
No new permission is introduced, so no `[negative-perm]` spec is required (A10 N/A);
no backend path is touched, so A9 is N/A.

## Unit — the normalizer

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: display `\[ … \]` becomes block math with the content on its own line — `'\\[ x^2 + y^2 = z^2 \\]'` → `'$$\nx^2 + y^2 = z^2\n$$'`; a multi-line body (`\frac{a}{b} \\` + `c = d`) keeps its internal newlines; and a doubly-escaped `'\\\\[ not math \\\\]'` is left unchanged by the opener lookbehind.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: inline `\( … \)` becomes `$…$` with NO newline injected — `'Energy \\( E = mc^2 \\) is nice.'` → `'Energy $E = mc^2$ is nice.'`, including inside a list item (`'- a \\( x \\) b'` → `'- a $x$ b'`), and `'a\\\\(b)'` (LaTeX row break followed by a paren) is left unchanged.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: block positioning and container continuation prefixes — mid-sentence `'Given \\[ E = mc^2 \\] we conclude.'` → `'Given \n$$\nE = mc^2\n$$\n we conclude.'`; bullet `'- first \\[ x_1 \\]\n- second'` → 2-space-prefixed fences; ordered `'1. step \\[ \\frac{a}{b} \\]'` → 3-space prefix; blockquote `'> quote \\[ x \\]\n> more'` → every emitted line carries `> `.
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: each guard returns the input unchanged or safely downgraded — blank line inside the delimiters is not converted; a body line that is exactly `$$` is not converted; a 4-space-indented `'    \\[ x \\]'` (indented code) is not converted; empty `'\\[\\]'` / `'\\(  \\)'` are not converted; a table row `'| a | \\[ x^2 \\] |'` downgrades to inline `'| a | $x^2$ |'`; inline whose inner contains `$` (`'\\( a $ b \\)'`) is not converted while display with `\$5` IS converted.
- **TEST-5** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: streaming safety and idempotence — an unclosed `'streaming \\[ \\frac{k}'` and `'partial \\( x'` pass through byte-identical; `'\\[ a \\] then \\[ b'` converts the complete pair and leaves the trailing partial untouched; pre-existing `'keep $x$ and $$y$$'` is untouched; and `f(f(x)) === f(x)` holds for every input used across TEST-1..5.

## Unit — the shared preprocessor (highest blast radius)

- **TEST-6** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/components/common/markdownPreprocess.test.ts` — asserts: (a) code protection — `\[ x \]` inside a fenced block and inside an inline code span are both left literal; (b) the math-span regression guard — `'$$ a[1] $$\n\n[1]: http://x'` leaves the math span untouched (proving the shortcut-reference regex no longer reaches inside math); (c) **byte-identical on non-math input** — a corpus of reference-link forms (full `[t][id]`, collapsed `[t][]`, shortcut `[t]`, a `[id]: url "title"` definition, a footnote `[^1]`, an array index, a fenced block, an inline span) produces exactly the same output as the pre-change implementation, captured as literal expected strings; (d) the widened early return still short-circuits a string with neither `[` nor `\(`.

## E2E — real rendering in the browser (B7: verification means running it)

- **TEST-7** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-6] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: an assistant message containing the ACTUAL issue-#177 equations (`\[ \frac{d^2C(x)}{dx^2} - \frac{k}{D}C(x) = 0 \]` and inline `\( x^2 \)`) renders KaTeX in the chat bubble — `.katex-display` count > 0 for the display equation and `.katex` count > 0 for the inline one — and the raw LaTeX source text is NOT present. This is the literal reproduction of the reported bug (rule B9).
- **TEST-8** (tier: e2e) [covers: ITEM-4, ITEM-5] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: a `\[ x \]` inside a fenced code block renders zero `.katex` elements and the literal `\[ x \]` text is still visible in the code block — the code-protection guarantee proven in the real renderer, not just the pure function.
- **TEST-9** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: the inverted former negative test — `$$x^2 + y^2 = z^2$$` in an assistant message now renders `.katex` (count > 0), pinning that the KaTeX plugin stays wired and that the retired `[[no-katex-remark-rehype]]` directive does not creep back.
- **TEST-10** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/skills/skill-detail-drawer.spec.ts` — asserts: the skill detail drawer renders a `\[ … \]` equation in the SKILL.md body as `.katex`, AND — the DEC-7 obligation (a) — that the two newly-inherited `preprocessMarkdown` behaviors work on this surface: a reference-style link `[docs][1]` + `[1]: https://…` resolves to a real anchor, and an external image degrades to the `🖼` placeholder rather than a broken caption.
- **TEST-11** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/workflows/run-step-expanders.spec.ts` — asserts: an expanded workflow step whose output is markdown containing `\[ … \]` renders `.katex`, and non-math step output (including a JSON payload, which short-circuits to `<pre>`) renders byte-identically to before — the no-regression half of the DEC-7 obligation on this surface.

## Regression suites that must stay green (blast-radius guard, ITEM-5/6)

These are not new tests; they are pre-existing suites the change must not break, and
they are recorded here because ITEM-5/6 alter code they exercise. Their results are
recorded in TEST_RESULTS.md alongside the enumerated tests.

- The FULL `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` suite (not only the
  math cases) — it covers reference links, images, tables, code fences and mermaid
  through the two chat renderers ITEM-6 changes.
- `npm run test:unit` in `src-app/ui` in its entirety — includes
  `citationTokenize.test.ts`, `footnoteScope.test.ts`,
  `modules/file/utils/markdownRoundtrip.test.ts`.
- `npm run check (ui): PASS` and `gate:ui (ui): PASS` — the static + runtime-health
  gates required for any UI diff.
