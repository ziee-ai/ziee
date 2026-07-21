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

// TEST-6 — `preprocessMarkdown` is shared by EVERY markdown surface (both chat
// renderers, the file viewer, the skill drawer, workflow step output), so the
// math pass added here must be provably additive: identical output for input
// that contains no math, and no bracket-rewriting reach into a math span.

test('math delimiters are never rewritten inside code', () => {
  // fenced block — the `\[` must survive as literal text for the code renderer
  assert.equal(preprocessMarkdown('```\n\\[ x \\]\n```'), '```\n\\[ x \\]\n```')
  assert.equal(preprocessMarkdown('~~~\n\\( x \\)\n~~~'), '~~~\n\\( x \\)\n~~~')
  // inline code span
  assert.equal(preprocessMarkdown('use `\\[ x \\]` here'), 'use `\\[ x \\]` here')
  assert.equal(preprocessMarkdown('use `\\( x \\)` here'), 'use `\\( x \\)` here')
})

test('math spans are protected from the reference-link pass', () => {
  // Regression guard: the shortcut-reference regex would otherwise rewrite the
  // `[1]` INSIDE the equation into a link, corrupting it. Pre-existing `$$…$$`
  // was already vulnerable to this before the math pass existed.
  assert.equal(
    preprocessMarkdown('$$ a[1] $$\n\n[1]: http://x'),
    '$$ a[1] $$\n\n[1]: http://x',
  )
  // ...and the same for a span this pass just produced from `\[ … \]`
  assert.equal(
    preprocessMarkdown('\\[ a[1] \\]\n\n[1]: http://x'),
    '$$\na[1]\n$$\n\n[1]: http://x',
  )
})

test('math delimiters outside code are converted', () => {
  assert.equal(
    preprocessMarkdown('Given \\[ E = mc^2 \\] we conclude.'),
    'Given \n$$\nE = mc^2\n$$\n we conclude.',
  )
  assert.equal(preprocessMarkdown('inline \\( x^2 \\) here'), 'inline $x^2$ here')
})

test('non-math input is unchanged by the math pass', () => {
  // Reference links: full, collapsed, shortcut — all still inline as before.
  assert.equal(
    preprocessMarkdown('full ref [text][id] here\n\n[id]: https://example.com'),
    'full ref [text](https://example.com) here\n\n[id]: https://example.com',
  )
  assert.equal(
    preprocessMarkdown('collapsed ref [text][] here\n\n[text]: https://example.com "Title"'),
    'collapsed ref [text](https://example.com "Title") here\n\n[text]: https://example.com "Title"',
  )
  assert.equal(
    preprocessMarkdown('shortcut ref [text] here\n\n[text]: https://example.com'),
    'shortcut ref [text](https://example.com) here\n\n[text]: https://example.com',
  )
  // Non-definitions stay put.
  assert.equal(
    preprocessMarkdown('unresolved [nope] with no definition'),
    'unresolved [nope] with no definition',
  )
  assert.equal(
    preprocessMarkdown('footnote ref[^1] stays\n\n[^1]: the note'),
    'footnote ref[^1] stays\n\n[^1]: the note',
  )
  assert.equal(
    preprocessMarkdown('array[i] and [TODO] and arr[1]'),
    'array[i] and [TODO] and arr[1]',
  )
  // Bracket rewriting still never reaches into code.
  assert.equal(
    preprocessMarkdown('```\narr[1] and [text]\n```\n\n[text]: http://x'),
    '```\narr[1] and [text]\n```\n\n[text]: http://x',
  )
  assert.equal(
    preprocessMarkdown('use `x[1]` and `[text]` now\n\n[text]: http://x'),
    'use `x[1]` and `[text]` now\n\n[text]: http://x',
  )
  // Image blocking is untouched. NOTE: `window` is absent under node:test, so
  // `isSameOriginImage` falls into its catch and treats an absolute URL as
  // external — deterministic here, and browser-faithful behavior is covered by
  // the e2e suite rather than reproduced with a stub.
  assert.equal(
    preprocessMarkdown('![alt](https://external.example/img.png)'),
    '[🖼 alt](https://external.example/img.png)',
  )
  assert.equal(preprocessMarkdown('![alt](/local/img.png)'), '![alt](/local/img.png)')
})

test('the early return still short-circuits delimiter-free input', () => {
  const plain = 'plain prose with no brackets at all'
  assert.equal(preprocessMarkdown(plain), plain)
  assert.equal(preprocessMarkdown(''), '')
  // ...but a string with `\(` and no `[` must NO LONGER short-circuit, or inline
  // math would never be converted (the bug the widened guard fixes).
  assert.equal(preprocessMarkdown('energy \\( E \\) here'), 'energy $E$ here')
})
