import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  reservedImageBox,
  toPositiveNumber,
  RESERVED_IMAGE_MIN_HEIGHT,
} from './reservedImageBox.ts'

// TEST-3: pure height-reservation for inline images (ITEM-3). No render.

test('intrinsic width+height → exact aspect-ratio box (stable before/after load)', () => {
  const before = reservedImageBox(800, 400, false)
  const after = reservedImageBox(800, 400, true)
  assert.equal(before.hasDims, true)
  assert.deepEqual(before.style, { aspectRatio: '800 / 400' })
  // Loaded state does not change a dimensioned box → zero post-load shift.
  assert.deepEqual(after.style, before.style)
})

test('parses string dimension attributes', () => {
  const box = reservedImageBox('640', '480', false)
  assert.deepEqual(box.style, { aspectRatio: '640 / 480' })
})

test('no dims + not loaded → reserves the default min-height', () => {
  const box = reservedImageBox(undefined, undefined, false)
  assert.equal(box.hasDims, false)
  assert.deepEqual(box.style, { minHeight: RESERVED_IMAGE_MIN_HEIGHT })
})

test('no dims + loaded → releases the reservation (final = natural height)', () => {
  const box = reservedImageBox(undefined, undefined, true)
  assert.equal(box.hasDims, false)
  assert.deepEqual(box.style, {})
})

test('partial / non-positive dims fall back to min-height reservation', () => {
  assert.deepEqual(reservedImageBox(800, undefined, false).style, {
    minHeight: RESERVED_IMAGE_MIN_HEIGHT,
  })
  assert.deepEqual(reservedImageBox(0, 0, false).style, {
    minHeight: RESERVED_IMAGE_MIN_HEIGHT,
  })
  assert.deepEqual(reservedImageBox('-5', '10', false).style, {
    minHeight: RESERVED_IMAGE_MIN_HEIGHT,
  })
})

test('toPositiveNumber', () => {
  assert.equal(toPositiveNumber(42), 42)
  assert.equal(toPositiveNumber('42'), 42)
  assert.equal(toPositiveNumber('42px'), 42)
  assert.equal(toPositiveNumber(0), undefined)
  assert.equal(toPositiveNumber(-1), undefined)
  assert.equal(toPositiveNumber('abc'), undefined)
  assert.equal(toPositiveNumber(undefined), undefined)
})
