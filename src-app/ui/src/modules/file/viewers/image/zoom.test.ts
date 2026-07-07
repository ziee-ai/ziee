import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  DEFAULT_IMAGE_VIEW,
  MAX_SCALE,
  MIN_SCALE,
  clampScale,
  clampTranslate,
  zoomStep,
} from './zoom.ts'

// ── TEST-1 (ITEM-1): clamp + step math ──────────────────────────────────────

test('DEFAULT_IMAGE_VIEW reproduces the pre-feature render (fit @ scale 1)', () => {
  assert.deepEqual(DEFAULT_IMAGE_VIEW, { scale: 1, mode: 'fit' })
})

test('clampScale pins to [MIN_SCALE, MAX_SCALE]', () => {
  assert.equal(clampScale(100), MAX_SCALE)
  assert.equal(clampScale(0.0001), MIN_SCALE)
  assert.equal(clampScale(2), 2)
})

test('clampScale collapses non-finite / non-positive to MIN_SCALE (never 0/NaN)', () => {
  assert.equal(clampScale(0), MIN_SCALE)
  assert.equal(clampScale(-3), MIN_SCALE)
  assert.equal(clampScale(Number.NaN), MIN_SCALE)
  assert.equal(clampScale(Number.POSITIVE_INFINITY), MAX_SCALE)
})

test('zoomStep multiplies then clamps and never yields 0/NaN', () => {
  assert.equal(zoomStep(1, 1.25), 1.25)
  assert.equal(zoomStep(1, 0.8), 0.8)
  // Repeated zoom-in saturates at MAX_SCALE, not beyond.
  assert.equal(zoomStep(MAX_SCALE, 1.25), MAX_SCALE)
  // Repeated zoom-out bottoms out at MIN_SCALE.
  assert.equal(zoomStep(MIN_SCALE, 0.8), MIN_SCALE)
  // A bad factor is a no-op on the clamped current scale.
  assert.equal(zoomStep(2, 0), 2)
  assert.equal(zoomStep(2, Number.NaN), 2)
})

// ── TEST-2 (ITEM-3): pan clamp ───────────────────────────────────────────────

test('clampTranslate pins pan to ±overflow/2 per axis', () => {
  // overflow 200 → max pan ±100.
  assert.deepEqual(clampTranslate(500, 0, 200, 0), { x: 100, y: 0 })
  assert.deepEqual(clampTranslate(-500, 0, 200, 0), { x: -100, y: 0 })
  assert.deepEqual(clampTranslate(40, -30, 200, 200), { x: 40, y: -30 })
})

test('clampTranslate pins an axis to 0 when the content fits (no overflow)', () => {
  assert.deepEqual(clampTranslate(50, 50, 0, 0), { x: 0, y: 0 })
  assert.deepEqual(clampTranslate(50, 50, -10, -10), { x: 0, y: 0 })
  // One axis overflows, the other fits.
  assert.deepEqual(clampTranslate(50, 50, 100, 0), { x: 50, y: 0 })
})

test('clampTranslate treats non-finite input as 0', () => {
  assert.deepEqual(clampTranslate(Number.NaN, Number.NaN, 200, 200), { x: 0, y: 0 })
})
