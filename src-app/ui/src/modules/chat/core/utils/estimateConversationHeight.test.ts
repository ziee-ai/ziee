import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { ConversationResponse } from '@/api-client/types.ts'
import {
  estimateConversationHeight,
  FLOOR,
} from './estimateConversationHeight.ts'

// TEST-1: content-aware first-pass conversation-row height estimate (ITEM-1).
// Pure, no DOM.

let n = 0
/** Minimal ConversationResponse (only title + message_count are read). */
function conv(
  over: Omit<Partial<ConversationResponse>, 'title'> & {
    title?: string | null
  } = {},
): ConversationResponse {
  return {
    id: `00000000-0000-0000-0000-${String(n++).padStart(12, '0')}`,
    title: 'Untitled',
    message_count: 0,
    updated_at: '2026-07-08T00:00:00Z',
    created_at: '2026-07-08T00:00:00Z',
    user_id: 'u1',
    ...over,
  } as ConversationResponse
}

const SHORT = 'Hi'
const LONG =
  'A deliberately long conversation title that will certainly wrap onto a ' +
  'second rendered line at the default content width so line-clamp-2 applies'

test('estimateConversationHeight: total — undefined/empty/whitespace → floor', () => {
  assert.equal(estimateConversationHeight(undefined), FLOOR)
  assert.equal(estimateConversationHeight(conv({ title: '' })), FLOOR)
  assert.equal(estimateConversationHeight(conv({ title: '   ' })), FLOOR)
  assert.equal(estimateConversationHeight(conv({ title: null })), FLOOR)
})

test('estimateConversationHeight: long (2-line) title taller than short', () => {
  const shortH = estimateConversationHeight(conv({ title: SHORT }))
  const longH = estimateConversationHeight(conv({ title: LONG }))
  assert.ok(longH > shortH, `expected ${longH} > ${shortH}`)
  assert.equal(shortH, FLOOR) // a 2-char title is always one line
})

test('estimateConversationHeight: message_count>0 never decreases (monotonic)', () => {
  const title =
    'A borderline-length conversation title around the one-line wrap boundary'
  const w = 520
  const without = estimateConversationHeight(conv({ title, message_count: 0 }), w)
  const withCount = estimateConversationHeight(
    conv({ title, message_count: 12 }),
    w,
  )
  assert.ok(withCount >= without, `expected ${withCount} >= ${without}`)
})

test('estimateConversationHeight: width-sensitive (narrower ≥ wider)', () => {
  const c = conv({ title: LONG })
  const narrow = estimateConversationHeight(c, 320)
  const wide = estimateConversationHeight(c, 1200)
  assert.ok(narrow >= wide, `expected ${narrow} >= ${wide}`)
})

test('estimateConversationHeight: caps at two lines (line-clamp-2)', () => {
  const huge = estimateConversationHeight(conv({ title: 'x'.repeat(5000) }))
  const twoLine = estimateConversationHeight(conv({ title: LONG }))
  assert.equal(huge, twoLine)
})

test('estimateConversationHeight: memoized per (conv, width bucket)', () => {
  const c = conv({ title: LONG })
  const a = estimateConversationHeight(c, 864)
  const b = estimateConversationHeight(c, 864)
  assert.equal(b, a)
  assert.equal(typeof estimateConversationHeight(c, 300), 'number')
})
