import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import { useMessageViewStateStore } from './MessageViewState.store.ts'
import {
  DEFAULT_MESSAGE_COLLAPSED,
  resolveFileState,
  resolveMessageCollapsed,
} from './messageViewState.helpers.ts'

// TEST-4 / TEST-5 (message-scroll-stability ITEM-4/5/6): the lifted view-state
// store round-trips + resets. Driven headless via getState() actions (no React).

const s = () => useMessageViewStateStore.getState()

beforeEach(() => {
  s().resetViewState()
})

test('setMessageCollapsed round-trips; unknown id defaults to collapsed', () => {
  assert.equal(resolveMessageCollapsed(s().collapsed, 'm1'), DEFAULT_MESSAGE_COLLAPSED)
  s().setMessageCollapsed('m1', false)
  assert.equal(s().collapsed['m1'], false)
  assert.equal(resolveMessageCollapsed(s().collapsed, 'm1'), false)
  s().setMessageCollapsed('m1', true)
  assert.equal(resolveMessageCollapsed(s().collapsed, 'm1'), true)
})

test('inline-file state round-trips (collapsed / seen / height) per key', () => {
  const k = 'ziee://chart.svg'
  // Absent → defaults.
  assert.deepEqual(resolveFileState(s().files, k), {
    collapsed: false,
    seen: false,
    heightPx: null,
  })
  s().setFileCollapsed(k, true)
  s().markFileSeen(k)
  s().setFileHeight(k, 512)
  assert.deepEqual(resolveFileState(s().files, k), {
    collapsed: true,
    seen: true,
    heightPx: 512,
  })
})

test('resetViewState clears both maps (conversation switch)', () => {
  s().setMessageCollapsed('m1', false)
  s().setFileHeight('ziee://x', 300)
  s().resetViewState()
  assert.deepEqual(s().collapsed, {})
  assert.deepEqual(s().files, {})
})

test('markFileSeen / setFileHeight seed a default entry before mutating', () => {
  // Mutating an absent key must not throw and must apply on top of defaults.
  s().markFileSeen('ziee://only-seen')
  assert.deepEqual(resolveFileState(s().files, 'ziee://only-seen'), {
    collapsed: false,
    seen: true,
    heightPx: null,
  })
})
