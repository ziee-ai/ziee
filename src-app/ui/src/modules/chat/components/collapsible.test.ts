import { test } from 'node:test'
import assert from 'node:assert/strict'
import { shouldOfferCollapse, COLLAPSE_CHAR_THRESHOLD } from './collapsible.ts'

test('offers collapse for long, non-streaming content', () => {
  assert.equal(
    shouldOfferCollapse({ length: COLLAPSE_CHAR_THRESHOLD + 1, isStreaming: false }),
    true,
  )
})

test('never offers collapse while streaming, even when long', () => {
  assert.equal(
    shouldOfferCollapse({ length: COLLAPSE_CHAR_THRESHOLD * 5, isStreaming: true }),
    false,
  )
})

test('does not offer collapse under the threshold', () => {
  assert.equal(shouldOfferCollapse({ length: 0, isStreaming: false }), false)
  assert.equal(
    shouldOfferCollapse({ length: COLLAPSE_CHAR_THRESHOLD, isStreaming: false }),
    false,
    'exactly at the threshold is not yet long enough',
  )
})
