import { test } from 'node:test'
import assert from 'node:assert/strict'
import { makeChatListMetrics } from './chatListMetrics.ts'

// TEST-3: the DEV correction-metrics surface (ITEM-6). Pure factory, no render.
// The DEV-gating + the RUNNING settle-to-~0 proof is TEST-6.

test('makeChatListMetrics: corrections is a LIVE view over the counter', () => {
  const counter = { corrections: 0 }
  const m = makeChatListMetrics(counter, () => 1234)
  assert.equal(m.corrections, 0)
  counter.corrections++
  counter.corrections++
  assert.equal(m.corrections, 2) // reads through, not a snapshot
})

test('makeChatListMetrics: reset() zeroes the counter', () => {
  const counter = { corrections: 7 }
  const m = makeChatListMetrics(counter, () => 0)
  m.reset()
  assert.equal(counter.corrections, 0)
  assert.equal(m.corrections, 0)
})

test('makeChatListMetrics: totalSize reads through the getter', () => {
  let size = 500
  const m = makeChatListMetrics({ corrections: 0 }, () => size)
  assert.equal(m.totalSize(), 500)
  size = 900
  assert.equal(m.totalSize(), 900)
})
