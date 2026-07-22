import { test } from 'node:test'
import assert from 'node:assert/strict'
import { normalizeMathDelimiters } from './normalizeMathDelimiters.ts'

// Every input used below, reused by the idempotence check in TEST-5.
const ALL_INPUTS: string[] = []
const check = (input: string, expected: string) => {
  ALL_INPUTS.push(input)
  assert.equal(normalizeMathDelimiters(input), expected)
}

// TEST-1 — `\[ … \]` becomes BLOCK math, content on its own line (remark-math
// parses `$$x$$` on one line as INLINE math, so the newlines are load-bearing).
test('display delimiters become block math', () => {
  check('\\[ x^2 + y^2 = z^2 \\]', '$$\nx^2 + y^2 = z^2\n$$')

  // a multi-line body keeps its internal newlines, incl. a LaTeX `\\` row break
  check(
    '\\[\n\\frac{a}{b} \\\\\nc = d\n\\]',
    '$$\n\\frac{a}{b} \\\\\nc = d\n$$',
  )

  // doubly-escaped opener is a literal backslash + bracket, not math
  check('literal \\\\[ not math \\\\]', 'literal \\\\[ not math \\\\]')
})

// TEST-1b — inline `\( … \)` becomes INLINE math `$…$`. No newline is injected:
// `$…$` is a text construct, unlike the display form.
test('inline delimiters become inline math', () => {
  check('Energy \\( E = mc^2 \\) is nice.', 'Energy $E = mc^2$ is nice.')
  check(
    'decay \\( \\lambda = \\sqrt{D/k} \\) here',
    'decay $\\lambda = \\sqrt{D/k}$ here',
  )
  // padding inside the delimiters is trimmed; text outside is preserved verbatim
  check('a \\(   x   \\) b', 'a $x$ b')
})

// TEST-2 — Flavor B: there is NO content/math-signal gate. A bare symbol and
// bare function notation are what models write most often in science prose, and
// a heuristic that demands `=` or a `\command` misses all of them.
test('bare symbols and function notation convert (no content gate)', () => {
  check('the coefficient \\(D\\) is fixed', 'the coefficient $D$ is fixed')
  check('let \\( C(x) \\) denote', 'let $C(x)$ denote')
  check('and \\(f(x)\\) too', 'and $f(x)$ too')
  check('- a \\( x \\) b', '- a $x$ b')
})

// TEST-3 — the regression trio, under Flavor B.
//
// Two of the three now render as math-italic. That is the accepted tradeoff, and
// it is NOT a corruption: in un-fenced prose markdown's own character-escape rule
// ALREADY strips `\(` → `(`, so `sed -e 's/\(foo\)/bar/'` renders today as
// `sed -e 's/(foo)/bar/'` — the backslashes are lost either way. Converting
// trades `(foo)` for an italic `foo`. A real sed command belongs in a code block,
// which `preprocessMarkdown` protects before this function is ever called (see
// markdownPreprocess.test.ts).
//
// The third — regex alternation — is caught by the BRE guard and stays EXACTLY as
// it renders today.
test('the regression trio behaves as Flavor B specifies', () => {
  check(
    "Ran: sed -e 's/\\(foo\\)/bar/' on 3 files",
    "Ran: sed -e 's/$foo$/bar/' on 3 files",
  )
  check('To escape use \\( and \\) in LaTeX.', 'To escape use $and$ in LaTeX.')
  check("grep -E '\\(x\\)' file.txt", "grep -E '$x$' file.txt")

  // ...but explicit regex syntax is left untouched, byte-for-byte as today
  check('Pattern \\(a\\|b\\) matched 4 lines', 'Pattern \\(a\\|b\\) matched 4 lines')
})

// TEST-4 — every BRE/ERE signal skips: alternation, backreference, interval,
// and the `\+` / `\?` quantifiers.
test('BRE signals in the body skip conversion', () => {
  check('alt \\(a\\|b\\) x', 'alt \\(a\\|b\\) x')
  // the signal has to be INSIDE the body to be seen — a trailing backreference
  // (`s/\(a\)\1/x/`, the usual shape) leaves the body a bare `a`, which converts
  check('backref \\(a\\1\\) x', 'backref \\(a\\1\\) x')
  check('interval \\(ab\\{2,3\\}\\) x', 'interval \\(ab\\{2,3\\}\\) x')
  check('plus \\(a\\+\\) x', 'plus \\(a\\+\\) x')
  check('opt \\(a\\?\\) x', 'opt \\(a\\?\\) x')
})

// TEST-5 — THE hijack guard. Injecting a `$` into a paragraph that already holds
// a live `$` lets the two pair up: `cost $5 and $E$ here` tokenizes as math
// "5 and " plus a dangling literal `E$ here`. That is real corruption, so the
// whole match is left alone. A `$…$` span crosses a plain newline, so the guard
// is paragraph-scoped, not line-scoped.
test('an unpaired $ in the paragraph blocks conversion', () => {
  check('cost $5 and \\( E=mc^2 \\) here', 'cost $5 and \\( E=mc^2 \\) here')
  check(
    'cost $5 line one\nand \\( E=mc^2 \\) here',
    'cost $5 line one\nand \\( E=mc^2 \\) here',
  )
  // a `$` AFTER the match blocks it just the same
  check('\\( E=mc^2 \\) then cost $5', '\\( E=mc^2 \\) then cost $5')
  // converting inside an existing `$…$` span would break that span
  check('see $a \\( b \\) c$ end', 'see $a \\( b \\) c$ end')
  // an even count still hijacks — micromark pairs left-to-right — so the rule is
  // "any live `$`", not "an odd number of them"
  check('$5 and $10 for \\( E \\)', '$5 and $10 for \\( E \\)')
})

// TEST-6 — ...but the guard is PARAGRAPH-scoped and escape-aware, so it does not
// suppress conversion further than it must.
test('the dollar guard is paragraph-scoped and escape-aware', () => {
  // a blank line ends the paragraph, so the `$5` cannot reach this match
  check('cost $5 para one\n\nand \\( E=mc^2 \\) here', 'cost $5 para one\n\nand $E=mc^2$ here')
  // an escaped `\$` never opens a math span, so it cannot hijack
  check('cost \\$5 and \\( E=mc^2 \\) here', 'cost \\$5 and $E=mc^2$ here')
  // ...but `\\$` is a literal backslash followed by a LIVE `$`, which does
  check('cost \\\\$5 and \\( E \\) here', 'cost \\\\$5 and \\( E \\) here')
})

// TEST-7 — body-shape guards, all degrading to "leave it exactly as it is".
test('inline body-shape guards leave unsafe cases untouched', () => {
  // empty delimiters are not math
  check('a \\(\\) b', 'a \\(\\) b')
  // a nested `\(` means the lazy closer matched the INNER `\)`
  check('x \\( a \\( b \\) c \\) y', 'x \\( a \\( b \\) c \\) y')
  // a `$` in the body would close the emitted span early
  check('a \\( x $ y \\) b', 'a \\( x $ y \\) b')

  // TWO ADJACENT pairs would emit `$a$$b$`. A math-text closer must be a `$` run
  // of the SAME length as its opener, so the inner `$$` does not close the first
  // span and the whole thing collapses into ONE span with the body `a$$b`.
  // Neither pair is safe alone, so both are skipped.
  check('x \\( a \\)\\( b \\) y', 'x \\( a \\)\\( b \\) y')
  check('\\(a\\)\\(b\\)', '\\(a\\)\\(b\\)')
  // ...but ANY separator between them is enough to make both safe
  check('x \\(a\\) \\(b\\) y', 'x $a$ $b$ y')
  check('x \\(a\\), \\(b\\) y', 'x $a$, $b$ y')
})

// TEST-8 — an indented code block is never touched. `preprocessMarkdown` splits
// out fenced + inline code before calling us; the 4-column indented block is the
// one code form that split cannot see, so it is guarded here.
test('indented code blocks are not converted inline', () => {
  check('    \\( x \\)', '    \\( x \\)')
  check('\t\\( x \\)', '\t\\( x \\)')
  // ...but a list item or blockquote at the same indent is NOT a code block
  check('  - a \\( x \\) b', '  - a $x$ b')
  check('> quote \\( x \\)', '> quote $x$')
})

// TEST-11 — display/inline ORDERING. The display pass runs first; the inline
// pass then sees the `$$` it emitted, so the paragraph guard declines to rewrite
// the `\(` riding along in the body. Were the order reversed (or the guard
// absent) this would emit `$$\na $b$ c\n$$`, which KaTeX cannot parse.
test('an inline pair inside a display block is left in the body', () => {
  check('\\[ a \\( b \\) c \\]', '$$\na \\( b \\) c\n$$')
})

// TEST-3 — block positioning: the `$$` must open at a line start, and every
// emitted line must carry its container's continuation prefix or the block
// escapes the list item / blockquote it belongs to.
test('display math reaches block position inside its container', () => {
  // mid-sentence — display math IS a block, so the sentence splits in two
  check(
    'Given \\[ E = mc^2 \\] we conclude.',
    'Given \n$$\nE = mc^2\n$$\n we conclude.',
  )

  // bullet list: marker becomes equivalent-width spaces (2 for `- `)
  check(
    '- first \\[ x_1 \\]\n- second',
    '- first \n  $$\n  x_1\n  $$\n- second',
  )

  // ordered list: 3 for `1. `
  check(
    '1. step \\[ \\frac{a}{b} \\]\n2. next',
    '1. step \n   $$\n   \\frac{a}{b}\n   $$\n2. next',
  )

  // blockquote markers carry over verbatim
  check('> quote \\[ x \\]\n> more', '> quote \n> $$\n> x\n> $$\n> more')
})

// TEST-4 — every guard degrades to "leave it exactly as it is" (or, for a table
// row, to inline math) — never to corrupted markdown.
test('guards leave unsafe cases untouched', () => {
  // a blank line can't be inside real math — it means an unclosed `\[` ran on
  check('\\[ a\n\nb \\]', '\\[ a\n\nb \\]')

  // a body line that is exactly `$$` would close the fence early
  check('\\[ a\n$$\nb \\]', '\\[ a\n$$\nb \\]')

  // 4-space indent with no bullet/quote is an indented code block — and a TAB
  // is 4 columns in CommonMark, so it is the same code block
  check('    \\[ x \\]', '    \\[ x \\]')
  check('\t\\[ x \\]', '\t\\[ x \\]')

  // inside a link destination/title a newline would break the link outright,
  // so it downgrades to inline rather than corrupting the syntax
  check('[t](http://x "\\[ y \\]")', '[t](http://x "$y$")')
  // ...but a COMPLETED link earlier on the line must not trigger that guard
  check('[t](http://x) then \\[ y \\]', '[t](http://x) then \n$$\ny\n$$')

  // empty delimiters are not math
  check('\\[\\]', '\\[\\]')

  // a table row is newline-terminated — downgrade to inline rather than break it
  check('| a | \\[ x^2 \\] |', '| a | $x^2$ |')

  // a nested `\[` means the lazy closer matched the INNER `\]`; converting would
  // leave a dangling literal `\]`, so the whole thing is left alone
  check('\\[ a \\[ b \\] c \\]', '\\[ a \\[ b \\] c \\]')

  // a display body containing `$` is safe — its closer must be a line of only `$$`
  check('\\[ \\text{cost} = \\$5 \\]', '$$\n\\text{cost} = \\$5\n$$')

  // ...but when a table/link downgrade forces the INLINE form, a `$` in the body
  // would close the span early, so that case stays literal
  check('| a | \\[ x $ y \\] |', '| a | \\[ x $ y \\] |')
})

// TEST-5 — streaming safety (an unclosed opener simply doesn't match, so the
// partial renders exactly as it does today) and idempotence.
test('partial and pre-existing math pass through unchanged', () => {
  check('streaming \\[ \\frac{k}', 'streaming \\[ \\frac{k}')

  // a display body longer than the ReDoS cap simply doesn't convert
  check(`\\[ ${'x'.repeat(2100)} \\]`, `\\[ ${'x'.repeat(2100)} \\]`)

  // the complete pair converts; the trailing partial is left for the next frame
  check('\\[ a \\] then \\[ b', '$$\na\n$$\n then \\[ b')

  // pre-existing `$` math is never touched
  check('keep $x$ and $$y$$', 'keep $x$ and $$y$$')

  // non-strings and delimiter-free strings short-circuit
  assert.equal(normalizeMathDelimiters(''), '')
  assert.equal(normalizeMathDelimiters('plain prose [1] and arr[0]'), 'plain prose [1] and arr[0]')
})

// TEST-9 — the same streaming/ReDoS contract for the INLINE pass. An unclosed
// opener simply fails to match, which is what makes this safe to run on every
// frame of a response.
test('inline partials and over-cap bodies pass through unchanged', () => {
  // mid-stream: the closer hasn't arrived yet
  check('streaming \\( E=', 'streaming \\( E=')

  // a doubly-escaped opener is a literal backslash + paren, not math
  check('a\\\\(b\\\\)', 'a\\\\(b\\\\)')

  // over the 300-char ReDoS cap it simply doesn't convert
  check(`a \\( ${'x'.repeat(310)} \\) b`, `a \\( ${'x'.repeat(310)} \\) b`)

  // an inline body may not span a line — a newline means an unclosed `\(` ran on
  check('a \\( x\ny \\) b', 'a \\( x\ny \\) b')

  // the complete pair converts; the trailing partial waits for the next frame
  check('\\( a \\) then \\( b', '$a$ then \\( b')
})

// TEST-10 — the two passes coexist without interfering.
test('display and inline math convert in the same document', () => {
  check(
    'Steady state:\n\n\\[ \\frac{d^2C}{dx^2} = 0 \\]\n\nwith \\( C(x) \\) bounded.',
    'Steady state:\n\n$$\n\\frac{d^2C}{dx^2} = 0\n$$\n\nwith $C(x)$ bounded.',
  )
})

// A CRLF document (an uploaded .md, a Windows-authored SKILL.md) must produce the
// same block structure as LF — no stray `\r` line after the closing fence, which
// is what the `\r?` in the trailing-context test prevents.
test('CRLF input produces the same block structure as LF', () => {
  check('Given \r\n\\[ x^2 \\]\r\nafter', 'Given \r\n$$\nx^2\n$$\r\nafter')
  check('- first \\[ x_1 \\]\r\n- second', '- first \n  $$\n  x_1\n  $$\r\n- second')
  // a CRLF blank line inside the delimiters still trips the runaway guard
  check('\\[ a\r\n\r\nb \\]', '\\[ a\r\n\r\nb \\]')
})

test('normalizeMathDelimiters is idempotent', () => {
  for (const input of ALL_INPUTS) {
    const once = normalizeMathDelimiters(input)
    assert.equal(normalizeMathDelimiters(once), once, `not idempotent for: ${JSON.stringify(input)}`)
  }
})
