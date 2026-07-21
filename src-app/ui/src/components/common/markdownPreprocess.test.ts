import { test } from 'node:test'
import assert from 'node:assert/strict'
import { preprocessMarkdown } from './markdownPreprocess.ts'

// A GFM footnote definition is `[^id]: body`, which is shape-identical to a link
// reference definition `[id]: url`. A co-located citation run `[^1][^2]` is in
// turn shape-identical to a reference link `[text][id]`. Collecting footnote
// definitions as link definitions therefore rewrote the run into `[^1](body)`,
// destroying the second citation of every run — the exact thing the paper-
// grouped reference work depends on (ziee#167).

test('leaves co-located footnote citations alone (short definition bodies)', () => {
  // Short, single-token bodies are what used to trip it: the url capture is
  // `\S+` anchored to end-of-line, so `Two.` matched and `Second body here.`
  // did not — the bug only bit some documents, which is why it went unnoticed.
  const md =
    'A[^1][^2][^3].\n\n[^1]: One.\n\n[^2]: Two.\n\n[^3]: Three.'
  assert.equal(preprocessMarkdown(md), md)
})

test('leaves paper-grouped citation runs alone', () => {
  const md =
    'A[^1-1][^1-2][^2].\n\n[^1-1]: One.\n\n[^1-2]: Two.\n\n[^2]: Three.'
  assert.equal(preprocessMarkdown(md), md)
})

test('still inlines a REAL cross-block reference link', () => {
  // The feature this function exists for must keep working: a `[text][id]`
  // usage whose `[id]: url` definition lives in another block.
  const md = 'See [the docs][d] for more.\n\n[d]: https://example.test/docs'
  assert.match(preprocessMarkdown(md), /\[the docs\]\(https:\/\/example\.test\/docs\)/)
})

test('still inlines a reference link with a title', () => {
  const md = 'See [it][d].\n\n[d]: https://example.test "Title"'
  assert.match(preprocessMarkdown(md), /\[it\]\(https:\/\/example\.test "Title"\)/)
})
