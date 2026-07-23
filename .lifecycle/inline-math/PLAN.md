# PLAN — Render inline LaTeX math `\( … \)` (Flavor B, aggressive)

Follow-up to #177 / #188. PR #188 shipped `normalizeMathDelimiters.ts`, which converts
**display** `\[ … \]` → `$$ … $$` but deliberately leaves inline `\( … \)` untouched,
citing collision with POSIX BRE grouping. Models emit inline math constantly
(`\( E=mc^2 \)`, `\( \lambda \)`, `\( D \)`, `\( C(x) \)`) and it renders as mangled
prose. This must be fixed **in code**, model-agnostically — a prompt-level fix is out of
scope.

**Flavor chosen by the human: B (aggressive).** Convert EVERY `\( … \)` that clears the
guards; no content/math-signal gate, so bare function notation (`\(C(x)\)`, `\(f(x)\)`)
and bare symbols (`\(D\)`) render too.

## Research basis (re-verified against the installed micromark)

Rendered through `micromark` + `micromark-extension-math` with
`singleDollarTextMath: true` (the config `streamdownPlugins.ts:35` passes):

- Un-fenced `\(` is **already** stripped to `(` by markdown's own character-escape rule:
  `sed -e 's/\(foo\)/bar/'` → `sed -e 's/(foo)/bar/'`; `\(a\|b\)` → `(a|b)`;
  `To escape use \( and \)` → `To escape use ( and )`; `\( E = mc^2 \)` → `( E = mc^2 )`.
  So converting these cannot make them *meaningfully* worse than today, and the only
  correct home for a real sed/regex command is a code block, which `preprocessMarkdown`
  already protects by splitting on fences + inline code before the math pass.
- **The one real corruption vector — the unpaired-`$` hijack.** `cost $5 and \( E \)`
  naively becomes `cost $5 and $E$`, and micromark pairs `$5 and $` as math, leaving a
  dangling literal `E$`. Also verified: a `$…$` span **does** cross a newline but **not**
  a blank line; `\$` never opens math; converting a `\(…\)` nested inside an existing
  `$…$` span breaks that span. The display path is immune to all of this (a `$$` run
  cannot pair with a lone `$`), so this is new surface area — ITEM-4 exists solely for it.

## Items

- **ITEM-1**: Add a second, inline pass to `normalizeMathDelimiters` that rewrites
  `\( … \)` → `$ … $`, running AFTER the existing display pass over its output. The
  display pass and every one of its guards are unchanged. Widen the module's fast-path
  short-circuit from `indexOf('\\[') === -1` to `\\[` **or** `\\(`. Flavor B: no
  content/math-signal gate — every match that clears ITEM-2..ITEM-6 converts.
- **ITEM-2**: Body-shape guards, each degrading to "leave the text exactly as today":
  empty body → skip; a nested `\(` in the body (the lazy closer matched an inner `\)`)
  → skip; a `$` inside the body (it would close the emitted span early) → skip.
- **ITEM-3**: BRE/regex-signal guard — skip when the body contains `\|`, `\1`–`\9`,
  `\{`, `\}`, `\+`, or `\?`. Keeps `Pattern \(a\|b\)` rendering byte-identically to today.
- **ITEM-4**: Unpaired-`$` paragraph guard — skip when the containing paragraph
  (blank-line-delimited, matching the `\n(?!\s*\n)` reasoning already documented on
  `MATH_SPAN_RE`, `markdownPreprocess.ts:32`) contains ANY un-escaped `$` outside the
  match. "Any" rather than "odd" because `$5 and $10 for \( E \)` has an even count and
  still hijacks; "any `$` in the paragraph" is the only rule that provably cannot change
  how pre-existing `$` tokens pair. This single guard covers prices, pre-existing
  `$…$`/`$$…$$` math, a `\(…\)` nested inside an existing `$…$` span, AND the `$$` block
  the display pass just emitted upstream — which is what keeps the existing expectation
  `'\[ a \( b \) c \]'` → `'$$\na \( b \) c\n$$'` green with no change.
- **ITEM-5**: Indented-code-block guard — reuse the existing `continuationPrefix`
  helper; a `null` return (4+ columns of indent, no bullet/quote) means an indented code
  block → skip.
- **ITEM-6**: Structural safety carried over from the display path: the inline matcher is
  single-line and length-capped (`[^\r\n]{0,300}?`) as a ReDoS bound, since this runs on
  every streaming frame; a `(?<!\\)` lookbehind rejects a doubly-escaped `\\(`; an
  unclosed `\( E=` simply fails to match, so streaming partials pass through; the pass is
  idempotent by construction because its output contains no `\(`.
- **ITEM-10**: Tighten ITEM-4's guard to pair `$` runs BY LENGTH instead of counting
  them. Added after live container verification (see DRIFT-3). The coarse "any live `$`
  blocks" rule also blocked on `$$`, and because the display pass deliberately emits its
  fences with SINGLE newlines (to keep the block inside its list item / blockquote), a
  converted display block stays in the SAME blank-line-delimited paragraph as the prose
  around it — so `The energy is \[ E=mc^2 \] where \( m \) is mass.` silently lost its
  `\( m \)`. Verified against micromark that a `$$` run can never close the single `$` we
  emit, so only two shapes are genuinely unsafe: the match sitting INSIDE an existing
  span, or an UNPAIRED single `$` loose in the paragraph. Everything else converts.
- **ITEM-9**: Adjacent-pair guard. Added in phase 7 (see DRIFT-2). Two `\( … \)` pairs
  with nothing between them — `\( a \)\( b \)` — would emit `$a$$b$`. A math-text closer
  must be a `$` run of the SAME length as its opener, so the inner `$$` does not close
  the first span and the whole run collapses into ONE span whose body is `a$$b`, which
  KaTeX rejects. Converting either pair alone is equally unsafe (it would still abut the
  other's literal delimiter), so skip both. The ITEM-4 paragraph guard cannot cover this:
  it looks for a `$` already present in the SOURCE, whereas this collision is created by
  the rewrite itself. Must be index-based, not `slice(0, offset)`, or the pass goes
  quadratic again.
- **ITEM-8**: Widen `preprocessMarkdown`'s own early return (`markdownPreprocess.ts:75`).
  It bails when the document contains no `[`, justified by the comment "`\[` contains
  `[`, so the original guard already admits every input the math pass could act on". That
  is true for display math and **false for inline**: `\(` contains no `[`, so a message
  like `Energy \( E = mc^2 \) is nice.` returns unchanged before the math pass ever runs
  and ITEM-1 would silently do nothing. Admit `\\(` as well. Found during the phase-2
  audit; without this the entire feature is a no-op for the most common real input.
- **ITEM-7**: Documentation truth — rewrite the `normalizeMathDelimiters.ts` module
  header (its "INLINE … IS DELIBERATELY NOT CONVERTED" rationale is now superseded and
  actively misleading) to record the Flavor-B tradeoff and the unpaired-`$` finding, and
  update `markdownPreprocess.ts`'s item-(3) doc comment (`:66-70`) which repeats the old
  claim. Amended after DRIFT-1.3: also correct the stale call-site claim in
  `markdownPreprocess.test.ts:39-41` ("the skill drawer, workflow step output" do NOT
  route through `preprocessMarkdown` — verified by grep; there are exactly three
  production call sites) and the spec-header claim in `markdown-rendering.spec.ts:40-42`.
  A comment asserting a false safety/scope property is worse than no comment.

## Files to touch

- `src-app/ui/src/components/common/normalizeMathDelimiters.ts` — main change
  (ITEM-1..ITEM-7)
- `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — rewrite TEST-2
  (which currently pins the OLD pass-through behavior) + add the guard tests
- `src-app/ui/src/components/common/markdownPreprocess.ts` — early-return widening
  (ITEM-8) + doc comment (ITEM-7)
- `src-app/ui/src/components/common/markdownPreprocess.test.ts` — fence/inline-code
  protection case; plus (amended after DRIFT-1.2) rewrite the TWO pre-existing tests that
  pin the old pass-through behavior at this level — `display math outside code is
  converted, inline is left alone` and `the early return still short-circuits
  delimiter-free input`, the latter of which explicitly asserted the ITEM-8 no-op as
  correct
- `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — rewrite the
  `leaves inline \( … \) untouched` spec to the new B behavior; add the inline-math
  positive spec; extend the code-fence spec with `\( y \)`

No changes needed to the renderers: `preprocessMarkdown` is already the single call site
(`markdownPreprocess.ts:99`), and both chat renderers
(`modules/chat/components/TextContent.tsx`,
`modules/chat/extensions/text/components/TextContent.tsx`) plus the file viewer
(`modules/file/viewers/markdown/body.tsx`) already route through it. Skill/workflow
surfaces stay out of scope, as in #188.

## Patterns to follow

- **The display pass in the same file is the reference implementation.** Mirror its
  idioms exactly: a module-level named regex const with a comment explaining the
  lookbehind and the length cap as a *ReDoS bound, not a style choice*; a guard chain
  where every branch `return whole` (degrade-don't-corrupt); `\r?` everywhere a newline
  is matched, because an uploaded `.md` may be CRLF. Reuse `continuationPrefix` for the
  indented-code-block check rather than re-deriving it.
- **`markdownPreprocess.ts:32` (`MATH_SPAN_RE`)** is the precedent for the
  paragraph-scoping rule in ITEM-4 — it already documents that math can span lines but
  not a blank line, and that being conservative there is deliberate.
- **`normalizeMathDelimiters.test.ts`** is the unit-test pattern: the `check(input,
  expected)` helper that pushes into `ALL_INPUTS`, plus the trailing idempotence test
  that replays every input. New cases go through `check` so they inherit the idempotence
  proof for free.
- **`tests/e2e/chat/markdown-rendering.spec.ts`** is the e2e pattern:
  `seedAssistantWithText(page, testInfra.baseURL, …)` + `assistantBubble(page)`, then
  `bubble.evaluate(el => el.querySelectorAll('.katex').length)` for math assertions.
  Note its existing precedent of NOT asserting the raw TeX is absent (KaTeX keeps it in a
  hidden `<annotation>` for screen readers).

## UI-surface checklist

This feature adds **no new UI surface** — no page, drawer, card, panel, route, component,
store, permission, migration, or API. It is a pure string-level pre-tokenizer change
inside an existing shared code path, and its entire user-visible effect is that text
already being rendered in the existing chat bubble and file-viewer body now renders some
spans as inline KaTeX instead of parenthesized prose. Consequently:

- **Precedent** — the twin is the display-math pass in the very same function, shipped in
  #188; it is mirrored idiom-for-idiom (guard chain, degrade-don't-corrupt, ReDoS cap).
- **Scale / cardinality** — unchanged; no list, no fetch, no pagination. The one scale
  concern is per-frame CPU during streaming, addressed by ITEM-6's single-line 300-char
  cap (the same bound and rationale as the display pass's 2000-char cap).
- **Device size / responsive** — inline KaTeX is inline text; it reflows with its
  paragraph exactly as the surrounding prose does. No new layout, no new breakpoint
  behavior. Long equations are a pre-existing property of the already-shipped `$…$`
  inline-math support, not introduced here.
- **Populated-render review** — the populated render IS the e2e spec: a seeded assistant
  message containing real inline math, asserted to produce `.katex` and not
  `.katex-display`.
- **User-visible progress / input economy / multi-instance / URL-as-view-into-focus /
  platform affordances** — not applicable; no work is ingested or produced, no input is
  collected, no view instance or URL is involved.

## Out of scope

- Skill and workflow rendering surfaces (kept out of scope in #188).
- Any prompt-level instruction telling models to emit `$…$` — explicitly rejected by the
  task as non-generalizing.
- Widening `\( … \)` support to multi-line bodies (ITEM-6 caps at one line deliberately:
  an inline equation never spans a line, and allowing it would let an unclosed `\(` run
  away across a document).
