import { test } from 'node:test'
import assert from 'node:assert/strict'
import { isOutsideWindow, planTearOff, runTearOffPlan } from './tearOff.ts'

// TEST-91/92 (split-chat ITEM-58): the pure geometry + decision for tear-off —
// dragging a conversation past the window edge opens it as its own window
// (desktop only, strict outside-the-edge trigger).

// window at screen origin (200,150), size 800x600 → covers x∈[200,1000), y∈[150,750)
const WIN = { screenX: 200, screenY: 150, outerWidth: 800, outerHeight: 600 }

test('a release INSIDE the window rect is not outside', () => {
  assert.equal(isOutsideWindow({ screenX: 600, screenY: 400 }, WIN), false)
  assert.equal(isOutsideWindow({ screenX: 200, screenY: 150 }, WIN), false, 'top-left origin is inside')
})

test('a release past any edge is outside', () => {
  assert.equal(isOutsideWindow({ screenX: 199, screenY: 400 }, WIN), true, 'left of the window')
  assert.equal(isOutsideWindow({ screenX: 600, screenY: 149 }, WIN), true, 'above the window')
  assert.equal(isOutsideWindow({ screenX: 1000, screenY: 400 }, WIN), true, 'at/after the right edge (exclusive)')
  assert.equal(isOutsideWindow({ screenX: 600, screenY: 750 }, WIN), true, 'at/after the bottom edge (exclusive)')
  assert.equal(isOutsideWindow({ screenX: 5, screenY: 5 }, WIN), true, 'far off the top-left corner')
})

test('planTearOff opens ONLY when outside AND desktop', () => {
  // outside + desktop → open
  assert.deepEqual(planTearOff({ isOutside: true, isDesktop: true, conversationId: 'c1' }), {
    open: true,
    conversationId: 'c1',
    closePaneId: null,
  })
  // outside but WEB → ignored (desktop-only, DEC-70)
  assert.equal(planTearOff({ isOutside: true, isDesktop: false, conversationId: 'c1' }).open, false)
  // desktop but released INSIDE → ignored (strict trigger, DEC-71)
  assert.equal(planTearOff({ isOutside: false, isDesktop: true, conversationId: 'c1' }).open, false)
})

test('a PANE source that tears off MOVES — closePaneId is the pane', () => {
  assert.deepEqual(
    planTearOff({ isOutside: true, isDesktop: true, conversationId: 'c1', paneId: 'p2' }),
    { open: true, conversationId: 'c1', closePaneId: 'p2' },
  )
})

test('a pane source released inside / on web does NOT close the pane', () => {
  assert.equal(
    planTearOff({ isOutside: false, isDesktop: true, conversationId: 'c1', paneId: 'p2' }).closePaneId,
    null,
  )
  assert.equal(
    planTearOff({ isOutside: true, isDesktop: false, conversationId: 'c1', paneId: 'p2' }).closePaneId,
    null,
  )
})

// --- runTearOffPlan: the decision→effect glue the hook runs (spied effects) ---
const spy = () => {
  const calls: unknown[][] = []
  const fn = (...args: unknown[]) => {
    calls.push(args)
  }
  return Object.assign(fn, { calls })
}

test('runTearOffPlan opens the window (with title) and closes a pane source', () => {
  const openWindow = spy()
  const closePane = spy()
  const acted = runTearOffPlan(
    { open: true, conversationId: 'c1', closePaneId: 'p2' },
    { openWindow, closePane, title: 'Hello' },
  )
  assert.equal(acted, true)
  assert.deepEqual(openWindow.calls, [['c1', { title: 'Hello' }]])
  assert.deepEqual(closePane.calls, [['p2']])
})

test('runTearOffPlan opens without closing when there is no pane source', () => {
  const openWindow = spy()
  const closePane = spy()
  runTearOffPlan(
    { open: true, conversationId: 'c1', closePaneId: null },
    { openWindow, closePane },
  )
  assert.equal(openWindow.calls.length, 1)
  assert.equal(closePane.calls.length, 0, 'no pane to close')
})

test('runTearOffPlan does nothing for an inactive plan (web / in-window)', () => {
  const openWindow = spy()
  const closePane = spy()
  const acted = runTearOffPlan(
    { open: false, conversationId: 'c1', closePaneId: null },
    { openWindow, closePane },
  )
  assert.equal(acted, false)
  assert.equal(openWindow.calls.length, 0)
  assert.equal(closePane.calls.length, 0)
})
