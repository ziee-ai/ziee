import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { MessageWithContent, MessageContent } from '@/api-client/types.ts'
import { estimateMessageHeight, FLOOR } from './estimateMessageHeight.ts'

// TEST-1: content-aware first-pass row-height estimate (ITEM-1). Pure, no DOM.

function msg(contents: Partial<MessageContent>[]): MessageWithContent {
  return {
    id: 'm1',
    role: 'assistant',
    originated_from_id: '',
    edit_count: 0,
    created_at: '',
    contents: contents.map((c, i) => ({
      id: `c${i}`,
      message_id: 'm1',
      content_type: c.content_type ?? 'text',
      content: c.content ?? ({ type: 'text', text: '' } as never),
      sequence_order: i,
      created_at: '',
      updated_at: '',
    })),
  }
}

const text = (t: string): Partial<MessageContent> => ({
  content_type: 'text',
  content: { type: 'text', text: t } as never,
})

test('returns the floor for an undefined or empty message (null-safe, total)', () => {
  assert.equal(estimateMessageHeight(undefined), FLOOR)
  assert.equal(estimateMessageHeight(msg([])), FLOOR)
  assert.doesNotThrow(() => estimateMessageHeight(undefined, 0))
})

test('keeps a short user turn near the historical 140 floor', () => {
  const short = estimateMessageHeight(msg([text('ok, thanks!')]))
  assert.ok(short >= FLOOR)
  assert.ok(short < FLOOR + 60, `short=${short}`)
})

test('estimates a table / image / code message taller than a short text turn', () => {
  const short = estimateMessageHeight(msg([text('hello there')]))
  const table = estimateMessageHeight(
    msg([text('| a | b |\n|---|---|\n| 1 | 2 |')]),
  )
  const image = estimateMessageHeight(msg([text('see ![chart](/c.png)')]))
  const code = estimateMessageHeight(msg([text('```js\nconst x = 1\n```')]))
  assert.ok(table > short + 200, `table=${table} short=${short}`)
  assert.ok(image > short + 150, `image=${image} short=${short}`)
  assert.ok(code > short + 100, `code=${code} short=${short}`)
})

test('grows monotonically with text length up to a per-block cap', () => {
  const oneLine = estimateMessageHeight(msg([text('x'.repeat(80))]))
  const paragraph = estimateMessageHeight(msg([text('x'.repeat(2000))]))
  const huge = estimateMessageHeight(msg([text('x'.repeat(200000))]))
  assert.ok(paragraph > oneLine)
  assert.ok(huge < 1100, `huge=${huge}`) // capped, not unbounded
})

test('adds for non-text heavy blocks (tool_use / image / file_attachment)', () => {
  // Base text long enough to clear the 140px floor, so the block ADD isn't
  // masked by the floor clamp.
  const baseText = text('word '.repeat(80))
  const base = estimateMessageHeight(msg([baseText]))
  assert.ok(base > 140, `base=${base} should clear the floor`)
  const withTool = estimateMessageHeight(
    msg([
      baseText,
      { content_type: 'tool_use', content: { type: 'tool_use' } as never },
    ]),
  )
  const withImageBlock = estimateMessageHeight(
    msg([
      baseText,
      { content_type: 'image', content: { type: 'image' } as never },
    ]),
  )
  assert.ok(withTool > base + 100, `withTool=${withTool} base=${base}`)
  assert.ok(withImageBlock > base + 150, `withImageBlock=${withImageBlock} base=${base}`)
})

test('a medium answer estimates into a sane range (sanity bound, not a real-height ratio)', () => {
  // ~600 chars at the app content width is ~8-10 rendered lines; this is a
  // coarse sanity bound on the estimator's own output (no real measured height
  // is available in a pure unit test — the estimate↔measured closeness is
  // asserted end-to-end by the geometry e2e spec).
  const est = estimateMessageHeight(msg([text('word '.repeat(120))]), 736)
  assert.ok(est > 150, `est=${est}`)
  assert.ok(est < 600, `est=${est}`)
})

test('a narrower width estimates at least as tall as a wide one', () => {
  const t = text('word '.repeat(200))
  const narrow = estimateMessageHeight(msg([t]), 360)
  const wide = estimateMessageHeight(msg([t]), 896)
  assert.ok(narrow >= wide, `narrow=${narrow} wide=${wide}`)
})
