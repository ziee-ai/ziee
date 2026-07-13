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
// A 100-char title that fits ONE line at a wide (inline-meta) width but wraps to
// TWO at a narrow (stacked-meta) width — used to prove width-sensitivity without
// saturating the 2-line cap at both ends (which LONG does).
const BOUNDARY = 'x'.repeat(100)

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

test('estimateConversationHeight: message_count>0 makes a boundary title wrap → taller (inline layout)', () => {
  // At a WIDE width (900px, ≥ sm so the meta is INLINE) a 100-char title fits on
  // ONE line without the count meta (avail ≈812px, ≈108 chars/line); WITH the
  // count chip the reserved meta narrows the title to ≈728px (≈97 chars/line) so
  // it wraps to TWO lines. STRICTLY taller with the count — proving the
  // meta-widening logic contributes (not a trivial 96>=96).
  const w = 900
  const without = estimateConversationHeight(
    conv({ title: BOUNDARY, message_count: 0 }),
    w,
  )
  const withCount = estimateConversationHeight(
    conv({ title: BOUNDARY, message_count: 12 }),
    w,
  )
  assert.ok(withCount > without, `expected ${withCount} > ${without}`)
})

test('estimateConversationHeight: width-sensitive — a 100-char title is STRICTLY taller narrow', () => {
  // Wide (1200px, inline meta): the 100-char title fits one line. Narrow (320px,
  // < sm so the meta STACKS below): the title wraps to two lines AND the stacked
  // meta adds its own row. A width-ignoring implementation would return the same
  // value at both, so this proves genuine width-sensitivity.
  const c = conv({ title: BOUNDARY })
  const narrow = estimateConversationHeight(c, 320)
  const wide = estimateConversationHeight(c, 1200)
  assert.ok(narrow > wide, `expected narrow ${narrow} > wide ${wide}`)
})

test('estimateConversationHeight: caps at two lines (line-clamp-2)', () => {
  const huge = estimateConversationHeight(conv({ title: 'x'.repeat(5000) }))
  const twoLine = estimateConversationHeight(conv({ title: LONG }))
  assert.equal(huge, twoLine)
})

test('estimateConversationHeight: stable per (conv, width bucket); buckets independent', () => {
  const c = conv({ title: BOUNDARY })
  // Repeated calls at the same width are stable.
  const wide = estimateConversationHeight(c, 864)
  assert.equal(estimateConversationHeight(c, 864), wide)
  const narrow = estimateConversationHeight(c, 300)
  assert.equal(estimateConversationHeight(c, 300), narrow)
  // Different buckets are computed independently — the memo does NOT collapse
  // them to one cached value. BOUNDARY is 1 line inline-wide but 2 lines +
  // stacked-meta narrow, so narrow is STRICTLY taller (a collapsed cache fails).
  assert.ok(narrow > wide, `narrow ${narrow} > wide ${wide}`)
  // A different object with the same title keys the WeakMap per-object (no
  // cross-object leak) and yields the same value.
  assert.equal(estimateConversationHeight(conv({ title: BOUNDARY }), 864), wide)
})
