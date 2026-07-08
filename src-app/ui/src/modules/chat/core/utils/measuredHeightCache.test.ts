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
