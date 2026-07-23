import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mathToPlainText } from './mathPlainText.ts'

// TEST-22 — the reported defect: a conversation title carrying the raw LaTeX the
// user typed. Titles never reach the markdown renderer, so the delimiters used to
// surface verbatim in the header, the sidebar row and the pane picker.
test('math delimiters become their plain-text reading', () => {
  assert.equal(
    mathToPlainText('Check: the energy is \\[ E = mc^2 \\] where \\( m \\) ...'),
    'Check: the energy is E = mc^2 where m ...',
  )
  // inline and display both unwrap to their CONTENT — the delimiters are syntax
  assert.equal(mathToPlainText('the coefficient \\(D\\) is fixed'), 'the coefficient D is fixed')
  assert.equal(mathToPlainText('\\[ E = mc^2 \\]'), 'E = mc^2')
  // padding inside the pair goes with it
  assert.equal(mathToPlainText('a \\(   x   \\) b'), 'a x b')
})

// TEST-23 — a label is one line. A `first_message_preview` is the user's raw
// message, so it can carry newlines, and unwrapping a display block leaves its
// padding behind.
test('whitespace collapses to a single line', () => {
  assert.equal(
    mathToPlainText('Given\n\n\\[\nE = mc^2\n\\]\n\nwe conclude'),
    'Given E = mc^2 we conclude',
  )
  assert.equal(mathToPlainText('  \\( x \\)  '), 'x')
})

// TEST-24 — anything that is not a balanced pair is an escaped punctuation
// character, so it unescapes to that character, which is what markdown's own
// character-escape rule does with it. This is what keeps a TRUNCATED title
// readable: the 50-char cut can land mid-pair and orphan an opener.
test('leftover escapes unescape instead of leaking a backslash', () => {
  assert.equal(mathToPlainText('truncated \\( m'), 'truncated ( m')
  assert.equal(mathToPlainText('sed -e \\(foo'), 'sed -e (foo')
  assert.equal(mathToPlainText('stray \\] end'), 'stray ] end')
})

// TEST-25 — the no-op path. A label with no backslash is the overwhelmingly
// common case and must come back byte-for-byte, including its whitespace.
test('labels without backslashes are untouched', () => {
  assert.equal(mathToPlainText('BRCA1 in Breast Cancer'), 'BRCA1 in Breast Cancer')
  assert.equal(mathToPlainText('spaced   out   title'), 'spaced   out   title')
  assert.equal(mathToPlainText(''), '')
  assert.equal(mathToPlainText('costs $5 and $10'), 'costs $5 and $10')
})

// TEST-26 — structural safety, mirroring the delimiters' own guards.
test('structural cases degrade rather than mangle', () => {
  // a doubly-escaped opener is a literal backslash, not a delimiter
  assert.equal(mathToPlainText('a\\\\(b\\\\)'), 'a\\\\(b\\\\)')
  // an over-cap body is not treated as a pair; the escapes still unwrap
  const long = 'x'.repeat(320)
  assert.equal(mathToPlainText(`a \\( ${long} \\) b`), `a ( ${long} ) b`)
  // idempotent — the output carries no delimiters, so a second pass is identity
  const once = mathToPlainText('the rate \\( k \\) and \\[ E = mc^2 \\]')
  assert.equal(mathToPlainText(once), once)
  assert.equal(once, 'the rate k and E = mc^2')
})
