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

test('estimateConversationHeight: message_count>0 makes a boundary title wrap → taller', () => {
  // A 50-char title at width 520: without the count meta the title fits on ONE
  // line (avail ≈432px, ≈57 chars/line); WITH the count chip the reserved meta
  // narrows the title to ≈348px (≈46 chars/line) so it wraps to TWO lines. The
  // estimate must therefore be STRICTLY taller with the count — proving the
  // meta-widening logic actually contributes (not a trivial 96>=96).
  const title = 'x'.repeat(50)
  const w = 520
  const without = estimateConversationHeight(conv({ title, message_count: 0 }), w)
  const withCount = estimateConversationHeight(
    conv({ title, message_count: 12 }),
    w,
  )
  assert.ok(withCount > without, `expected ${withCount} > ${without}`)
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

test('estimateConversationHeight: stable per (conv, width bucket); buckets independent', () => {
  const c = conv({ title: LONG })
  // Repeated calls at the same width are stable (the memo returns a consistent
  // value; a broken cache that returned a stale wrong value would be caught by
  // the cross-bucket check below, since 300 and 864 must differ for a 2-line
  // title).
  const wide = estimateConversationHeight(c, 864)
  assert.equal(estimateConversationHeight(c, 864), wide)
  const narrow = estimateConversationHeight(c, 300)
  assert.equal(estimateConversationHeight(c, 300), narrow)
  // Different buckets are computed independently and correctly ordered — the
  // memo does NOT collapse them to one cached value.
  assert.ok(narrow >= wide, `narrow ${narrow} >= wide ${wide}`)
  // A different object with the same title keys the WeakMap per-object (no
  // cross-object leak) and yields the same value.
  assert.equal(estimateConversationHeight(conv({ title: LONG }), 864), wide)
})
