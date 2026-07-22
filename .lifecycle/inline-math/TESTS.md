# TESTS — inline math (Flavor B)

Bipartite map: every ITEM-1..8 is covered by ≥1 TEST; every TEST names its items, tier,
file and assertion. Unit cases go through the existing `check(input, expected)` helper so
they are additionally replayed by the file's idempotence test (TEST-12).

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: a genuine inline equation converts — `Energy \( E = mc^2 \) is nice.` → `Energy $E = mc^2$ is nice.`, and `\( \lambda = \sqrt{D/k} \)` → `$\lambda = \sqrt{D/k}$`; the body is trimmed and no newline is injected (inline math is NOT block math)
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: **Flavor B has no content gate** — bare symbols and function notation convert: `\(D\)` → `$D$`, `\( C(x) \)` → `$C(x)$`, `\( f(x) \)` → `$f(x)$`, `- a \( x \) b` → `- a $x$ b`. This is the whole point of choosing B over a math-signal heuristic
- **TEST-3** (tier: unit) [covers: ITEM-1, ITEM-3] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: **the rewritten regression trio.** `sed -e 's/\(foo\)/bar/'` and `To escape use \( and \)` now convert to `$foo$` / `$and$` (the accepted B tradeoff — no worse than markdown's pre-existing backslash strip, which already renders them `(foo)` / `( and )`), while `Pattern \(a\|b\)` stays **byte-identical to today** because guard 4 catches `\|`. Replaces the old TEST-2 block that pinned pass-through
- **TEST-4** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: every BRE/regex signal skips — `\|`, a backreference `\1`, an interval `\{2,3\}`, `\+`, `\?` each leave the whole match untouched
- **TEST-5** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: **the unpaired-`$` hijack guard.** `cost $5 and \( E=mc^2 \) here` is left untouched; so is the same text with the `$5` on a preceding line of the SAME paragraph (a `$…$` span crosses a newline); and `see $a \( b \) c$ end` (converting would break the enclosing span). Without this guard the first case renders as math `5 and ` plus a dangling literal `E$`
- **TEST-6** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: the guard is **paragraph**-scoped, not document-scoped — a `$5` separated from the match by a blank line does NOT block conversion; and an escaped `\$5` does not block it either (an escaped `$` never opens math)
- **TEST-7** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: body-shape guards leave the text untouched — empty `\(\)`, a nested `\( a \( b \) c \)`, and a body containing `$` (which would close the emitted span early)
- **TEST-8** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: an indented code block is never touched — `    \( x \)` (4 spaces) and `\t\( x \)` (a tab = 4 CommonMark columns) both pass through, while a list/blockquote line at the same indent still converts
- **TEST-9** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: structural safety — an unclosed streaming partial `\( E=` passes through; a doubly-escaped `a\\(b\\)` is not math (lookbehind); a body over the 300-char cap does not convert; a body containing a newline does not convert; a complete pair converts while a trailing partial is left for the next frame
- **TEST-10** (tier: unit) [covers: ITEM-1, ITEM-6] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: pre-existing math is untouched — `keep $x$ and $$y$$` is unchanged, and the whole existing display-math suite (`\[ … \]` → `$$ … $$`, container prefixes, CRLF, guards) still passes unmodified
- **TEST-11** (tier: unit) [covers: ITEM-1, ITEM-4] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: **display/inline ordering.** `\[ a \( b \) c \]` still yields exactly `$$\na \( b \) c\n$$` — the inline pass runs second and its paragraph guard sees the just-emitted `$$`, so it does not rewrite a `\(` that belongs to a display body (which would produce KaTeX-unparseable `$$\na $b$ c\n$$`)
- **TEST-12** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: idempotence — `normalizeMathDelimiters(f(x)) === f(x)` for EVERY input used anywhere in the file, via the existing `ALL_INPUTS` replay loop (new cases are registered automatically by `check`)
- **TEST-13** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/components/common/markdownPreprocess.test.ts` — asserts: **through the real entry point** `preprocessMarkdown` (not the normalizer) — `Energy \( E = mc^2 \) is nice.`, a document containing NO `[` at all, still converts. This is the only test that can catch the ITEM-8 early-return no-op; every normalizer-level test bypasses it
- **TEST-14** (tier: unit) [covers: ITEM-2, ITEM-8] file: `src-app/ui/src/components/common/markdownPreprocess.test.ts` — asserts: code protection holds through `preprocessMarkdown` — `\(foo\)` inside a fenced block and inside an inline-code span is left literal, while a `\( E=mc^2 \)` in the surrounding prose of the same document converts
- **TEST-15** (tier: e2e) [covers: ITEM-1, ITEM-8] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: a seeded assistant message `The decay length is \( \lambda = \sqrt{D/k} \) at steady state.` renders **inline** KaTeX in the real browser — `.katex` count > 0 AND `.katex-display` count === 0 — proving the end-to-end path (seed → store → TextContent → preprocessMarkdown → Streamdown → remark-math → rehype-katex → DOM), not just the string transform
- **TEST-16** (tier: e2e) [covers: ITEM-1, ITEM-3] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: **the rewritten prose spec.** The sed and escape-prose strings now render as inline KaTeX (`.katex` count > 0), documenting the deliberate Flavor-B tradeoff; and `Pattern \(a\|b\)` in the same message still renders as literal text `(a|b)` with no equation, proving guard 4 survives into the browser. Replaces `leaves inline \( … \) untouched so sed/grep prose survives`
- **TEST-17** (tier: e2e) [covers: ITEM-2, ITEM-5] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: extends the existing code-fence spec — `\( y \)` inside a ```tex fence and inside an inline-code span stays literal with `.katex` count === 0, alongside the existing `\[ x^2 \]` assertions
- **TEST-18** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: the unpaired-`$` guard in the real renderer — a message `That costs $5 and the rate \( k \) is fixed.` renders the literal text `$5` and `(k)` with NO mangled span (`.katex` count === 0), i.e. the hijack does not reach the DOM
- **TEST-19** (tier: e2e) [covers: ITEM-1, ITEM-7] file: `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` — asserts: the existing display-math spec (issue #177) is unregressed — `.katex-display` count === 2 and 2 hidden `<annotation>` elements, run against the now-two-pass normalizer

- **TEST-20** (tier: unit) [covers: ITEM-9] file: `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` — asserts: two directly adjacent pairs are BOTH skipped — `x \( a \)\( b \) y` and `\(a\)\(b\)` are unchanged (converting would emit `$a$$b$`, which collapses into a single span with the body `a$$b`) — while any separator makes both safe: `x \(a\) \(b\) y` → `x $a$ $b$ y` and `x \(a\), \(b\) y` → `x $a$, $b$ y`

## Coverage map (every ITEM → ≥1 TEST)

| ITEM | Covered by |
|---|---|
| ITEM-1 inline pass | TEST-1, TEST-2, TEST-3, TEST-10, TEST-11, TEST-15, TEST-16, TEST-19 |
| ITEM-2 body-shape guards | TEST-7, TEST-14, TEST-17 |
| ITEM-3 BRE/regex guard | TEST-3, TEST-4, TEST-16 |
| ITEM-4 unpaired-`$` guard | TEST-5, TEST-6, TEST-11, TEST-18 |
| ITEM-5 indented-code guard | TEST-8, TEST-17 |
| ITEM-6 structural safety | TEST-9, TEST-10, TEST-12 |
| ITEM-7 documentation truth | TEST-19 (pins the behavior the rewritten comments describe) |
| ITEM-8 caller early return | TEST-13, TEST-14, TEST-15 |
| ITEM-9 adjacent-pair guard | TEST-20 |

## Notes on tiering

The frontend e2e requirement is satisfied: TEST-15..TEST-19 are `tier: e2e` against
`src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts`. No `[negative-perm]` spec is
required — A10 is not engaged, because the diff introduces no permission (no
`modules/*/permissions.rs` change, no migration grant); see PLAN_AUDIT.md.

No test is mocked at anything but the real boundary: the unit tier exercises the real
transform, and the e2e tier drives the real browser render through the real component
chain. TEST-13/14 exist specifically because the unit tier CAN be green while the feature
is a user-facing no-op (the ITEM-8 trap).
