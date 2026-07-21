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

// TEST-2 — `\( … \)` becomes INLINE math, with no newline injected.
test('inline delimiters become inline math', () => {
  check('Energy \\( E = mc^2 \\) is nice.', 'Energy $E = mc^2$ is nice.')
  check('- a \\( x \\) b', '- a $x$ b')

  // `\\` is a LaTeX row break; the `(` after it does not open inline math
  check('a\\\\(b)', 'a\\\\(b)')
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

  // 4-space indent with no bullet/quote is an indented code block
  check('    \\[ x \\]', '    \\[ x \\]')

  // empty delimiters are not math
  check('\\[\\]', '\\[\\]')
  check('\\(  \\)', '\\(  \\)')

  // a table row is newline-terminated — downgrade to inline rather than break it
  check('| a | \\[ x^2 \\] |', '| a | $x^2$ |')

  // a `$` inside INLINE content would close the span early
  check('\\( a $ b \\)', '\\( a $ b \\)')

  // ...but display is safe, because its closer must be a line of only `$$`
  check('\\[ \\text{cost} = \\$5 \\]', '$$\n\\text{cost} = \\$5\n$$')
})

// TEST-5 — streaming safety (an unclosed opener simply doesn't match, so the
// partial renders exactly as it does today) and idempotence.
test('partial and pre-existing math pass through unchanged', () => {
  check('streaming \\[ \\frac{k}', 'streaming \\[ \\frac{k}')
  check('partial \\( x', 'partial \\( x')

  // the complete pair converts; the trailing partial is left for the next frame
  check('\\[ a \\] then \\[ b', '$$\na\n$$\n then \\[ b')

  // pre-existing `$` math is never touched
  check('keep $x$ and $$y$$', 'keep $x$ and $$y$$')

  // non-strings and delimiter-free strings short-circuit
  assert.equal(normalizeMathDelimiters(''), '')
  assert.equal(normalizeMathDelimiters('plain prose [1] and arr[0]'), 'plain prose [1] and arr[0]')
})

test('normalizeMathDelimiters is idempotent', () => {
  for (const input of ALL_INPUTS) {
    const once = normalizeMathDelimiters(input)
    assert.equal(normalizeMathDelimiters(once), once, `not idempotent for: ${JSON.stringify(input)}`)
  }
})
