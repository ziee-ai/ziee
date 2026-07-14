import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  acquireRecordingLock,
  releaseRecordingLock,
  getRecordingOwner,
} from './voiceRecordingLock.ts'

// TEST-68 (ITEM-45 / DEC-61 A1): the exclusive recording lock. The mic is single
// hardware, so only ONE split pane can own the recorder at a time — a second
// pane's acquire is refused (A1 disable-others, not supersede). Single-pane (null)
// never takes the lock (there is no other pane to exclude).

test('a split pane acquires; another pane is refused until release', () => {
  assert.equal(getRecordingOwner(), null)
  assert.equal(acquireRecordingLock('pane-A'), true)
  assert.equal(getRecordingOwner(), 'pane-A')
  assert.equal(acquireRecordingLock('pane-B'), false, 'B refused while A owns')
  assert.equal(acquireRecordingLock('pane-A'), true, 'A re-acquires its own lock')
  releaseRecordingLock('pane-B') // wrong owner → no-op
  assert.equal(getRecordingOwner(), 'pane-A')
  releaseRecordingLock('pane-A')
  assert.equal(getRecordingOwner(), null)
  assert.equal(acquireRecordingLock('pane-B'), true, 'B acquires after A releases')
  releaseRecordingLock('pane-B')
})

test('single-pane (null) never takes the lock', () => {
  assert.equal(acquireRecordingLock(null), true)
  assert.equal(getRecordingOwner(), null, 'single-pane sets no owner')
  releaseRecordingLock(null) // no-op
  assert.equal(getRecordingOwner(), null)
})
