import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MAX_ZOOM, MIN_ZOOM, ZOOM_STEPS, nextZoomStep } from './zoom.ts'

// TEST-3 (covers ITEM-7): the discrete zoom-step ladder.

test('zoom-in returns the next-larger step', () => {
  assert.equal(nextZoomStep(1.0, 1), 1.25)
  assert.equal(nextZoomStep(0.75, 1), 1.0)
})

test('zoom-out returns the next-smaller step', () => {
  assert.equal(nextZoomStep(1.0, -1), 0.75)
  assert.equal(nextZoomStep(1.25, -1), 1.0)
})

test('a scale between steps snaps to the correct neighbour', () => {
  // 1.1 is between 1.0 and 1.25
  assert.equal(nextZoomStep(1.1, 1), 1.25)
  assert.equal(nextZoomStep(1.1, -1), 1.0)
})

test('clamps to the ladder bounds', () => {
  assert.equal(nextZoomStep(MAX_ZOOM, 1), MAX_ZOOM)
  assert.equal(nextZoomStep(MIN_ZOOM, -1), MIN_ZOOM)
  assert.equal(nextZoomStep(99, 1), MAX_ZOOM)
  assert.equal(nextZoomStep(0.01, -1), MIN_ZOOM)
})

test('actual-size step (1.0) is on the ladder', () => {
  assert.ok((ZOOM_STEPS as readonly number[]).includes(1.0))
})
