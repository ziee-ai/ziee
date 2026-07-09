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

// Probe EVERY GFM construct the canvas claims to support. Each is a separate test
// so one drop doesn't hide the others — a failure means a missing Plate plugin
// (Plate DROPS unmodeled constructs, it does not preserve them verbatim).
const survives = (md: string, needle: RegExp) => () => {
  const out = editorToMarkdown(markdownToEditor(md))
  assert.match(out, needle, `dropped on round-trip; got:\n${out}`)
}

test('gfm: ordered list', survives('1. one\n2. two\n', /1\. one/))
// Plate emits `*` bullets (both are valid GFM; Streamdown renders them the same).
test('gfm: nested list', survives('- a\n  - b\n', /^\s+[-*] b/m))
test('gfm: strikethrough', survives('~~gone~~\n', /~~gone~~/))
test('gfm: inline code', survives('use `foo()` here\n', /`foo\(\)`/))
test('gfm: link', survives('[site](https://x.com)\n', /\[site\]\(https:\/\/x\.com\)/))
test(
  'gfm: image',
  survives('![alt](https://x.com/i.png)\n', /!\[alt\]\(https:\/\/x\.com\/i\.png\)/),
)
test('gfm: task list', survives('- [ ] todo\n- [x] done\n', /\[[ x]\] (todo|done)/))
test(
  'gfm: table',
  survives('| A | B |\n|---|---|\n| 1 | 2 |\n', /\| A \| B \|/),
)
