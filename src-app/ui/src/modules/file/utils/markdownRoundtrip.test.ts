import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  editorToMarkdown,
  markdownToEditor,
  normalizeMarkdown,
} from './markdownRoundtrip.ts'

const SAMPLE = `# Methods

Samples were prepared with **care** and precision.

- RNA extraction
- Reverse transcription

> Note: keep on ice.

\`\`\`python
print("hi")
\`\`\`
`

// Runtime proof that Plate's headless deserializeMd/serializeMd actually execute
// under node (not just typecheck) and preserve the GFM subset the canvas edits.

test('markdownToEditor produces a non-empty Plate value', () => {
  const v = markdownToEditor(SAMPLE)
  assert.ok(Array.isArray(v), 'value is an array of nodes')
  assert.ok(v.length > 0, 'value has at least one block')
})

test('editorToMarkdown round-trips the core GFM constructs', () => {
  const out = editorToMarkdown(markdownToEditor(SAMPLE))
  assert.match(out, /# Methods/, 'heading')
  assert.match(out, /\*\*care\*\*/, 'bold mark')
  assert.match(out, /RNA extraction/, 'list item text')
  assert.match(out, />\s*Note/, 'blockquote')
  assert.match(out, /```/, 'fenced code')
  assert.match(out, /print\("hi"\)/, 'code content')
})

test('normalizeMarkdown is idempotent', () => {
  const once = normalizeMarkdown(SAMPLE)
  const twice = normalizeMarkdown(once)
  assert.equal(twice, once, 're-normalizing is a no-op (stable diffs)')
})

test('empty input is safe', () => {
  assert.equal(typeof editorToMarkdown(markdownToEditor('')), 'string')
})
