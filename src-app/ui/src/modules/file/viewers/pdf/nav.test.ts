import { test } from 'node:test'
import assert from 'node:assert/strict'
import { clampPage, parseJump } from './nav.ts'

// TEST-5 (covers ITEM-6): jump-to-page clamp/parse.

test('clampPage clamps to [1, numPages]', () => {
  assert.equal(clampPage(0, 10), 1)
  assert.equal(clampPage(5, 10), 5)
  assert.equal(clampPage(99, 10), 10)
  assert.equal(clampPage(-3, 10), 1)
  assert.equal(clampPage(3.9, 10), 3) // truncates
  assert.equal(clampPage(Number.NaN, 10), 1)
})

test('parseJump parses a plain positive integer, clamped', () => {
  assert.equal(parseJump('3', 10), 3)
  assert.equal(parseJump('  7 ', 10), 7)
  assert.equal(parseJump('0', 10), 1)
  assert.equal(parseJump('50', 10), 10)
})

test('parseJump rejects non-integer input with null', () => {
  assert.equal(parseJump('abc', 10), null)
  assert.equal(parseJump('', 10), null)
  assert.equal(parseJump('1.5', 10), null)
  assert.equal(parseJump('-2', 10), null)
})
