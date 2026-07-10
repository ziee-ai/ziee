import { test } from 'node:test'
import assert from 'node:assert/strict'
import { claimSubscription, percentOf } from './downloadProgress.helpers.ts'

// ── SSE subscribe dedupe: the synchronous placeholder prevents a double-sub ────

test('claimSubscription grants the first claim and dedupes the second (same key)', () => {
  const aborts = new Map<string, AbortController>()
  // First caller claims the key…
  assert.equal(claimSubscription(aborts, 'whisper@v1@cpu'), true)
  // …and the slot is written SYNCHRONOUSLY (before any async controller arrives),
  // so a rapid second caller is deduped rather than opening a second stream.
  assert.equal(aborts.has('whisper@v1@cpu'), true)
  assert.equal(claimSubscription(aborts, 'whisper@v1@cpu'), false)
  // Only one entry exists for the key.
  assert.equal(aborts.size, 1)
})

test('claimSubscription treats distinct keys independently', () => {
  const aborts = new Map<string, AbortController>()
  assert.equal(claimSubscription(aborts, 'whisper@v1@cpu'), true)
  assert.equal(claimSubscription(aborts, 'whisper@v1@cuda'), true)
  assert.equal(aborts.size, 2)
})

test('claimSubscription re-grants after the entry is torn down', () => {
  const aborts = new Map<string, AbortController>()
  assert.equal(claimSubscription(aborts, 'k'), true)
  aborts.delete('k') // complete/failed handler removes the entry
  assert.equal(claimSubscription(aborts, 'k'), true, 're-subscribe allowed after teardown')
})

// ── progress percent clamp ────────────────────────────────────────────────────

test('percentOf returns undefined when total is unknown or zero', () => {
  assert.equal(percentOf(10, undefined), undefined)
  assert.equal(percentOf(10, 0), undefined)
})

test('percentOf computes and clamps into 0..100', () => {
  assert.equal(percentOf(0, 100), 0)
  assert.equal(percentOf(50, 100), 50)
  assert.equal(percentOf(100, 100), 100)
  // A received count exceeding total (retry/overcount) clamps to 100, not >100.
  assert.equal(percentOf(150, 100), 100)
})
