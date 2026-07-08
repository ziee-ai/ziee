import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  INLINE_FILE_DEFAULT_GENERIC_PX,
  INLINE_FILE_DEFAULT_TABULAR_PX,
  INLINE_FILE_MIN_PX,
  clampReservedPx,
  defaultReservedPx,
  maxReservedPx,
  resolveBodyHeightPx,
} from './inlineFileHeight.ts'

// TEST-2 (message-scroll-stability ITEM-2): pure reserved-height math. The
// defining property is that the skeleton and the mounted body resolve to the
// SAME px per viewer type, so the lazy body-mount is zero-delta.

test('defaultReservedPx is per viewer type', () => {
  assert.equal(defaultReservedPx(false), INLINE_FILE_DEFAULT_GENERIC_PX)
  assert.equal(defaultReservedPx(true), INLINE_FILE_DEFAULT_TABULAR_PX)
})

test('resolveBodyHeightPx returns the SAME value for skeleton and body', () => {
  const vh = 900
  for (const inlineFill of [false, true]) {
    // Skeleton and body both call this with the same (inlineFill, resizedPx).
    const skeleton = resolveBodyHeightPx(inlineFill, null, vh)
    const body = resolveBodyHeightPx(inlineFill, null, vh)
    assert.equal(skeleton, body)
    assert.equal(body, defaultReservedPx(inlineFill))
  }
})

test('resolveBodyHeightPx honors a persisted resized px (clamped)', () => {
  const vh = 1000
  assert.equal(resolveBodyHeightPx(false, 500, vh), 500)
  // Below floor → clamped up.
  assert.equal(resolveBodyHeightPx(false, 10, vh), INLINE_FILE_MIN_PX)
  // Above 80vh → clamped down to the max.
  assert.equal(resolveBodyHeightPx(false, 99999, vh), maxReservedPx(vh))
})

test('clampReservedPx bounds within [min, 80vh] and never below the generic default', () => {
  assert.equal(clampReservedPx(0, 1000), INLINE_FILE_MIN_PX)
  assert.equal(clampReservedPx(1_000_000, 1000), 800) // 0.8 * 1000
  // A tiny window still permits at least the generic default height.
  assert.equal(maxReservedPx(200), INLINE_FILE_DEFAULT_GENERIC_PX)
  assert.equal(clampReservedPx(1_000_000, 200), INLINE_FILE_DEFAULT_GENERIC_PX)
})
