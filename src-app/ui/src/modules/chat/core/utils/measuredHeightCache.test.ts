import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import {
  widthBucket,
  getMeasuredHeight,
  setMeasuredHeight,
  recordMeasurements,
  buildInitialMeasurementsCache,
  __clearMeasuredHeightCache,
} from './measuredHeightCache.ts'

// TEST-2: width-bucketed measured-height cache (ITEM-2). Pure, no DOM.

beforeEach(() => __clearMeasuredHeightCache())

test('stores and reads a height by (id, width bucket)', () => {
  setMeasuredHeight('a', 736, 420)
  assert.equal(getMeasuredHeight('a', 736), 420)
})

test('misses at a different width bucket (stale-width guard)', () => {
  setMeasuredHeight('a', 736, 420)
  // 736 → bucket 6, 360 → bucket 3.
  assert.equal(getMeasuredHeight('a', 360), undefined)
})

test('still hits within the same bucket for a sub-bucket width delta', () => {
  setMeasuredHeight('a', 720, 400)
  assert.equal(widthBucket(720), widthBucket(740))
  assert.equal(getMeasuredHeight('a', 740), 400)
})

test('ignores non-positive sizes', () => {
  setMeasuredHeight('a', 736, 0)
  setMeasuredHeight('a', 736, -5)
  assert.equal(getMeasuredHeight('a', 736), undefined)
})

test('recordMeasurements folds the virtualizer size map at the current width', () => {
  const itemSizeCache = new Map<string | number, number>([
    ['a', 300],
    ['b', 500],
    [7, 999], // numeric key (fallback index) — skipped, not a message id
  ])
  recordMeasurements(itemSizeCache, 736)
  assert.equal(getMeasuredHeight('a', 736), 300)
  assert.equal(getMeasuredHeight('b', 736), 500)
})

test('builds an initialMeasurementsCache only for ids with a cached height', () => {
  setMeasuredHeight('a', 736, 300)
  setMeasuredHeight('c', 736, 700)
  const seed = buildInitialMeasurementsCache(['a', 'b', 'c'], 736)
  assert.deepEqual(
    seed.map(s => s.key),
    ['a', 'c'],
  )
  const a = seed.find(s => s.key === 'a')!
  assert.equal(a.size, 300)
  assert.equal(a.index, 0)
  assert.equal(typeof a.start, 'number')
  assert.equal(typeof a.end, 'number')
  assert.equal(typeof a.lane, 'number')
})

test('returns an empty seed at a bucket with no cached heights', () => {
  setMeasuredHeight('a', 736, 300)
  assert.deepEqual(buildInitialMeasurementsCache(['a'], 120), [])
})

test('evicts nothing under the cap; keeps a re-set entry fresh', () => {
  setMeasuredHeight('x', 736, 100)
  setMeasuredHeight('x', 736, 100) // unchanged — refreshes recency, no dup
  assert.equal(getMeasuredHeight('x', 736), 100)
})

// ── ITEM-2 (chats-page-virtualization): the SAME id-generic cache is reused for
// conversation rows (DEC-2). Conversation ids are UUIDs — an opaque string key,
// disjoint from message ids — so no message-path change is needed. These cases
// exercise the cache through conversation-UUID keys to prove the reuse.

const CONV_A = '11111111-1111-1111-1111-111111111111'
const CONV_B = '22222222-2222-2222-2222-222222222222'

test('reuse: round-trips conversation-UUID keys at a width bucket', () => {
  setMeasuredHeight(CONV_A, 864, 76)
  assert.equal(getMeasuredHeight(CONV_A, 864), 76)
})

test('reuse: buildInitialMeasurementsCache seeds conv ids with heights, omits uncached', () => {
  setMeasuredHeight(CONV_A, 864, 76)
  setMeasuredHeight(CONV_B, 864, 96)
  const seed = buildInitialMeasurementsCache([CONV_A, 'uncached-conv', CONV_B], 864)
  assert.deepEqual(
    seed.map(s => s.key),
    [CONV_A, CONV_B],
  )
  assert.equal(seed.find(s => s.key === CONV_B)!.size, 96)
})

test('reuse: a different width bucket misses stale conversation heights', () => {
  setMeasuredHeight(CONV_A, 864, 76) // 864 → bucket 7
  assert.equal(getMeasuredHeight(CONV_A, 320), undefined) // 320 → bucket 3
})
