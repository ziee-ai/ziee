# PLAN — render-equations (issue #177)

## Problem + root cause

Chat model output containing display math renders as raw LaTeX. Issue #177's
screenshot shows `[ \frac{d^2C(x)}{dx^2} - \frac{k}{D}C(x) = 0 ]` — the leading
backslash is already gone, which is the tell: the model emitted `\[ … \]`,
markdown consumed `\[` as a **character escape** yielding a literal `[`, and the
LaTeX body leaked through as text. Inline `\( … \)` fails identically.

Verified in-tree (read, not inferred):

- Math already renders. `src-app/ui/src/components/common/streamdownPlugins.ts`
  wires `math: createMathPlugin({ singleDollarTextMath: true })`; `src/index.css:9`
  imports `katex/dist/katex.min.css`. `$…$` and `$$…$$` work today.
- `@streamdown/math@1.0.2` = `remark-math` + `rehype-katex`; its entire options
  surface is `{ singleDollarTextMath, errorColor }` — **no delimiter config**.
- `micromark-extension-math/dev/lib/syntax.js` is hard-wired to `codes.dollarSign`
  (`{flow: {[codes.dollarSign]: mathFlow}, text: {[codes.dollarSign]: mathText}}`).
  No hook exists for `\[` / `\(`.
- No delimiter normalization exists anywhere in the tree.

**Why a raw-string pre-transform and not a remark plugin.** `remark-math` is a
*micromark syntax extension* — it acts during tokenization. Any tree-level remark
plugin runs **after** markdown has parsed the text, by which point LaTeX is already
mangled (`x_1 … y_1` became emphasis, backslash escapes were consumed). The
transform must therefore run on the raw string before Streamdown — the same layer
as the existing `citationTokenize` / `preprocessMarkdown` pre-tokenizers. This is
both the correct layer and the established repo pattern.

Two parser mechanics this design depends on, both verified against the installed
parser rather than assumed:

1. `$$x$$` on one line yields **`inlineMath`** (`math-inline`), not display. Flow
   math requires the content on its **own line** — hence `$$\n…\n$$`.
2. `$$` at a line start **does interrupt an open paragraph** (`mathFlow` is
   `concrete: true`, no interrupt opt-out), so a single `\n` suffices — no blank
   line. That is what makes list / blockquote nesting survivable.

## Items

- **ITEM-1**: New pure module `normalizeMathDelimiters.ts` converting display `\[ … \]` to block math `$$\n…\n$$`, via a single combined global regex `/(?<!\\)\\\[([\s\S]*?)\\\]|(?<!\\)\\\(([\s\S]*?)\\\)/g` whose opener lookbehind rejects a doubly-escaped `\\[` and whose closer is deliberately unguarded so `\[ x \\\]` (LaTeX row break before the closer) still matches.
- **ITEM-2**: Same module converts inline `\( … \)` to `$…$` with no newline injection.
- **ITEM-3**: Container-aware block positioning — compute `lineHead` (last `\n` before the match → the match) and a continuation prefix (leading indent verbatim, blockquote `> ` markers verbatim, list marker replaced by equal-width spaces); emit the open fence, body and close fence each carrying the prefix; prepend `\n`+prefix only when `lineHead` is non-blank, append `\n`+prefix only when non-whitespace follows on the same line. Guarantees display math lands in block position inside paragraphs, bullet lists, ordered lists and blockquotes without breaking the container.
- **ITEM-4**: Safety guards in the same module — (A) inner containing a blank line is left unchanged (bounds an unclosed `\[` from swallowing paragraphs); (B) a display body line that is exactly `$$` is left unchanged; (C) a match on a line containing `|` (table row) downgrades to inline `$…$` rather than injecting a newline; a 4+-space indent with no bullet/quote is an indented code block and is skipped; empty inner is left unchanged; inline whose inner contains `$` is left unchanged (converting would close the span early and corrupt the line).
- **ITEM-5**: Wire the normalizer into the shared `preprocessMarkdown` as step 3 — widen the early return (`\(` contains no `[`), run math first inside the existing code-fence split loop, then split each segment again on math spans (`/(\$\$[\s\S]*?\$\$|\$[^$\n]*\$)/`) and run the image / reference-link passes only on non-math sub-segments. The nested split is required, not cosmetic: the shortcut-reference regex can today rewrite bracket-bearing LaTeX inside `$$ a[1] $$` when a `[1]: url` definition exists — a latent bug this closes.
- **ITEM-6**: Route `modules/chat/components/TextContent.tsx` through `preprocessMarkdown`, bringing it into exact lockstep with the `extensions/text` variant (which already calls it).
- **ITEM-7**: Route `modules/skill/components/SkillDetailDrawer.tsx` (both call sites) and `modules/workflow/components/StepOutputExpander.tsx` through `preprocessMarkdown`. Deliberately broader than #177 — these surfaces also gain reference-link inlining and blocked-image placeholders (both already ship on the file-viewer path). Carries two obligations: verify BOTH added behaviors render correctly on these two surfaces, and call the broadening out in a dedicated "Beyond #177" PR section.
- **ITEM-8**: Retire the stale `[[no-katex-remark-rehype]]` directive — invert the `does NOT render math with KaTeX styling` test in `tests/e2e/chat/markdown-rendering.spec.ts` and fix its two comment sites. That test asserts the opposite of shipped code (math IS wired, KaTeX CSS IS imported) and would fail regardless of this change.
- **ITEM-9**: [DESCOPED] Bare `\begin{equation}…\end{equation}` / `\begin{align}…\end{align}` environment support. See DEC-6 + the approved DESCOPED disposition in DECISIONS.md.

## Files to touch

**New**
- `src-app/ui/src/components/common/normalizeMathDelimiters.ts`
- `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts`
- `src-app/ui/src/components/common/markdownPreprocess.test.ts`

**Modified**
- `src-app/ui/src/components/common/markdownPreprocess.ts`
- `src-app/ui/src/modules/chat/components/TextContent.tsx`
- `src-app/ui/src/modules/skill/components/SkillDetailDrawer.tsx`
- `src-app/ui/src/modules/workflow/components/StepOutputExpander.tsx`
- `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts`
- `src-app/ui/tests/e2e/skills/skill-detail-drawer.spec.ts` (added at DRIFT-1.1 —
  TEST-10 lives here; it is the existing spec that already route-mocks the SKILL.md
  body, so it is the right host for ITEM-7's skill-surface proof)
- `src-app/ui/tests/e2e/workflows/run-step-expanders.spec.ts` (added at DRIFT-1.1 —
  TEST-11 lives here; it already owns the seeded dev-workflow `mock:` flow that lets
  a step's REAL output carry arbitrary markdown, with nothing mocked at the HTTP layer)

**Deliberately NOT touched**
- `src-app/ui/src/components/common/streamdownPlugins.ts` and
  `modules/chat/core/utils/chatMarkdownPlugins.ts` — no new plugin, no new
  dependency. The fix is entirely upstream of Streamdown.
- `modules/chat/extensions/text/components/TextContent.tsx` and
  `modules/file/viewers/markdown/body.tsx` — already call `preprocessMarkdown`,
  so they inherit math support with no edit.

## Patterns to follow

- **Pure helper in its own module + colocated `node:test` unit test** — mirror
  `modules/chat/core/utils/citationTokenize.ts` + `citationTokenize.test.ts`
  exactly (and `modules/chat/components/collapsible.ts` + `collapsible.test.ts`):
  `import { test } from 'node:test'`, `import assert from 'node:assert/strict'`,
  source imported with an explicit `.ts` extension, no `describe` block. The
  runner is `node:test` (`npm run test:unit`), NOT vitest — `vitest.config.ts`
  is scoped to `src/**/*.store.test.ts` only.
- **Split-on-code-fences-then-rewrite-even-segments** — the idiom in both
  `citationTokenize.ts` (`split(/(```[\s\S]*?```|`[^`]*`)/)`) and
  `markdownPreprocess.ts` (`split(/(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\r\n]+`)/)`
  with an `i += 2` parity loop). ITEM-5 extends the latter, it does not replace it.
- **Lookbehind in a tokenizer regex** — precedent is `citationTokenize.ts`'s
  `(?<![\w\]])`, so no browser-support question.
- **E2E** — `seedAssistantWithText(page, baseURL, markdown)` + `assistantBubble(page)`
  helpers already in `tests/e2e/chat/markdown-rendering.spec.ts`; add cases to that
  spec rather than creating a parallel one.

## UI-surface checklist

This feature adds **no new UI surface** — no page, drawer, card or panel. It changes
how already-existing markdown surfaces render one class of input. The checklist is
therefore answered as follows:

- **Precedent** — N/A (no new surface). The behavioral precedent is the existing
  `$…$` / `$$…$$` math path, which this makes reachable from LaTeX delimiters.
- **Scale / cardinality** — unchanged; no new list or collection. The normalizer is
  O(n) in message length with a cheap `indexOf` early return, run on text already
  being fully scanned by two sibling passes.
- **Device size / responsive** — a KaTeX display block is wider than prose and can
  overflow at ~390px. Every affected container already sets `w-full overflow-x-auto`
  (both `TextContent.tsx`, `file/viewers/markdown/body.tsx`), so a wide equation
  scrolls within its bubble rather than forcing horizontal page scroll. Verified at
  390px as part of ITEM-7's render check.
- **Populated-render review** — the gallery already has a populated math cell
  (`RENDERING_SHOWCASE_ID` / `deep-chat-rendering-showcase`, feeding the L1 math
  detector). Real equations from #177 are used for the manual check, not lorem.
- **User-visible progress** — N/A, synchronous string transform.
- **Input economy** — N/A, no new input.
- **JTBD** — "As a user asking a technical question, I want the model's equations to
  render as formatted math so I can read them, regardless of which delimiter
  convention the model happened to emit." Surfaces: chat assistant message (primary,
  both renderers), file markdown viewer, skill detail drawer, workflow step output.
  Streaming is part of the job: a half-arrived equation must degrade to today's
  plain text, never to a corrupted render.
- **Multi-instance / URL-as-view / platform affordances** — N/A.
