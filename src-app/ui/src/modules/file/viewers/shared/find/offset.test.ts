import { test } from 'node:test'
import assert from 'node:assert/strict'
import { locateSegment } from './offset.ts'

// TEST-12 (ITEM-4): the global-offset → text-segment mapping that lets a match
// spanning multiple text nodes resolve its start/end node independently.

test('locateSegment finds the containing segment', () => {
  // Three segments starting at 0, 5, 12 (lengths 5, 7, …).
  const starts = [0, 5, 12]
  assert.equal(locateSegment(starts, 0), 0)
  assert.equal(locateSegment(starts, 4), 0)
  assert.equal(locateSegment(starts, 5), 1) // boundary → the later segment
  assert.equal(locateSegment(starts, 11), 1)
  assert.equal(locateSegment(starts, 12), 2)
  assert.equal(locateSegment(starts, 99), 2) // past the end → last segment
})

test('a match spanning two segments maps its endpoints to different segments', () => {
  // "hel|lo world" split as ["hel"(0), "lo world"(3)]; the match "hello" is
  // start=0 (seg 0) .. end=5 (seg 1) → a cross-node Range.
  const starts = [0, 3]
  assert.equal(locateSegment(starts, 0), 0) // 'h' in seg 0
  assert.equal(locateSegment(starts, 5), 1) // end offset lands in seg 1
})

test('locateSegment returns -1 when there are no segments', () => {
  assert.equal(locateSegment([], 0), -1)
})

test('an offset before the first segment start pins to segment 0', () => {
  assert.equal(locateSegment([2, 6], 0), 0)
})
