import { test } from 'node:test'
import assert from 'node:assert/strict'
import { citationTokenize, isCitationHref } from './citationTokenize.ts'

// TEST-59 (FB-11): bare `[n]` KB citations become chip links, but real links,
// footnotes, non-numeric brackets, and already-tokenized markers are untouched.
test('citationTokenize rewrites only bare numeric [n]', () => {
  assert.equal(
    citationTokenize('It is in the chloroplast [1] and mitochondria [12].'),
    'It is in the chloroplast [1](#kb-cite-1) and mitochondria [12](#kb-cite-12).',
  )
  // real markdown link — the `(` lookahead protects it
  assert.equal(citationTokenize('see [the docs](https://x.y)'), 'see [the docs](https://x.y)')
  // footnote ref — has `^`, never matches
  assert.equal(citationTokenize('a claim[^1]'), 'a claim[^1]')
  // non-numeric bracket
  assert.equal(citationTokenize('array[i] and [TODO]'), 'array[i] and [TODO]')
  // NUMERIC array index (word-char before `[`) — left alone, not a citation
  assert.equal(citationTokenize('arr[1] and list[0]'), 'arr[1] and list[0]')
  // reference-style link usage/definition — untouched
  assert.equal(citationTokenize('[Smith][1] and [1]: http://x'), '[Smith][1] and [1]: http://x')
  // inside a code span / fenced block — never rewritten (would corrupt code)
  assert.equal(citationTokenize('use `x[1]` now'), 'use `x[1]` now')
  assert.equal(citationTokenize('```py\narr[1]\n```'), '```py\narr[1]\n```')
  // idempotent — an already-tokenized citation is left alone
  assert.equal(citationTokenize('[1](#kb-cite-1)'), '[1](#kb-cite-1)')
})

test('isCitationHref parses the chip href', () => {
  assert.equal(isCitationHref('#kb-cite-3'), 3)
  assert.equal(isCitationHref('#kb-cite-42'), 42)
  assert.equal(isCitationHref('#section'), null)
  assert.equal(isCitationHref('https://x.y'), null)
  assert.equal(isCitationHref(undefined), null)
})
