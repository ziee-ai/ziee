import { test } from 'node:test'
import assert from 'node:assert/strict'
import { zoneForX, planSinglePaneDrop, planSplitPaneDrop } from './singlePaneDrop.ts'

// TEST-88/89 (split-chat ITEM-57): the pure geometry + placement for single-pane
// edge-directional drop. `zoneForX` maps a pointer x into left/center/right
// thirds; `planSinglePaneDrop` turns a (zone, current, dropped) into the exact
// workspace mutation ConversationPage applies via the SplitView store.

// --- zoneForX: thirds of a 300px-wide container at left=100 (so 100..400) ---
test('zoneForX splits a container into left/center/right thirds', () => {
  const L = 100
  const W = 300 // thirds at x=200 and x=300
  assert.equal(zoneForX(100, L, W), 'left', 'far left edge')
  assert.equal(zoneForX(199, L, W), 'left', 'just before the 1/3 boundary')
  assert.equal(zoneForX(250, L, W), 'center', 'middle')
  assert.equal(zoneForX(399, L, W), 'right', 'just inside the right edge')
  assert.equal(zoneForX(400, L, W), 'right', 'far right edge')
})

test('zoneForX clamps an x outside the rect to the nearest edge zone', () => {
  assert.equal(zoneForX(-50, 100, 300), 'left', 'left of the rect')
  assert.equal(zoneForX(9999, 100, 300), 'right', 'right of the rect')
})

test('zoneForX with zero width resolves to center (no divide-by-zero)', () => {
  assert.equal(zoneForX(100, 100, 0), 'center')
})

// --- planSinglePaneDrop: current view = X, dropping Y ---
test('left zone splits [dropped, current] (new pane on the LEFT)', () => {
  assert.deepEqual(planSinglePaneDrop('left', 'X', 'Y'), {
    kind: 'split',
    order: ['Y', 'X'],
  })
})

test('right zone splits [current, dropped] (new pane on the RIGHT)', () => {
  assert.deepEqual(planSinglePaneDrop('right', 'X', 'Y'), {
    kind: 'split',
    order: ['X', 'Y'],
  })
})

test('center zone replaces the current conversation with the dropped one', () => {
  assert.deepEqual(planSinglePaneDrop('center', 'X', 'Y'), {
    kind: 'replace',
    id: 'Y',
  })
})

test('dropping a conversation onto its OWN view is a no-op in every zone', () => {
  for (const z of ['left', 'center', 'right'] as const) {
    assert.deepEqual(planSinglePaneDrop(z, 'X', 'X'), { kind: 'noop' })
  }
})

test('an empty dropped id is a no-op (defensive)', () => {
  assert.deepEqual(planSinglePaneDrop('left', 'X', ''), { kind: 'noop' })
})

// --- planSplitPaneDrop: dropping Z onto a pane holding A, in an existing split ---
test('split pane: left → insertBefore, right → insertAfter, center → replace', () => {
  assert.deepEqual(planSplitPaneDrop('left', 'A', 'Z', false), { kind: 'insertBefore' })
  assert.deepEqual(planSplitPaneDrop('right', 'A', 'Z', false), { kind: 'insertAfter' })
  assert.deepEqual(planSplitPaneDrop('center', 'A', 'Z', false), { kind: 'replace' })
})

test('split pane: at MAX_PANES cap, the insert edges fall back to replace', () => {
  assert.deepEqual(planSplitPaneDrop('left', 'A', 'Z', true), { kind: 'replace' })
  assert.deepEqual(planSplitPaneDrop('right', 'A', 'Z', true), { kind: 'replace' })
  assert.deepEqual(planSplitPaneDrop('center', 'A', 'Z', true), { kind: 'replace' })
})

test('split pane: dropping a conversation onto its OWN pane is a no-op', () => {
  for (const z of ['left', 'center', 'right'] as const) {
    assert.deepEqual(planSplitPaneDrop(z, 'A', 'A', false), { kind: 'noop' })
  }
  assert.deepEqual(planSplitPaneDrop('left', 'A', '', false), { kind: 'noop' })
})
