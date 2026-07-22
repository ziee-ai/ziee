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

// TEST-2 — inline `\( … \)` is DELIBERATELY NOT converted. It is byte-identical
// to POSIX BRE group syntax, and no structural rule separates a sed command from
// real math — `To escape use \( and \)` is whitespace-padded exactly like
// `\( E = mc^2 \)`. Converting it would delete the backslashes and italicise
// ordinary prose. Inline math still renders via `$…$`.
test('inline delimiters pass through untouched', () => {
  // the shell/regex family — the reason this branch does not exist
  check(
    "Ran: sed -e 's/\\(foo\\)/bar/' on 3 files",
    "Ran: sed -e 's/\\(foo\\)/bar/' on 3 files",
  )
  check('Pattern \\(a\\|b\\) matched 4 lines', 'Pattern \\(a\\|b\\) matched 4 lines')
  check("grep -E '\\(x\\)' file.txt", "grep -E '\\(x\\)' file.txt")

  // prose documenting LaTeX escaping — padded exactly like real math, which is
  // why no heuristic could have told them apart
  check('To escape use \\( and \\) in LaTeX.', 'To escape use \\( and \\) in LaTeX.')

  // ...and genuine inline math is left alone too: that is the accepted tradeoff
  check('Energy \\( E = mc^2 \\) is nice.', 'Energy \\( E = mc^2 \\) is nice.')
  check('- a \\( x \\) b', '- a \\( x \\) b')
  check('a\\\\(b)', 'a\\\\(b)')

  // an inline pair INSIDE a display block still rides along in the body, since
  // only the outer display delimiters are rewritten
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
