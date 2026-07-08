import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  anchorRestoreNeeded,
  indexRestoreOffset,
  pickTopAnchor,
  restoreDelta,
  type MessageBox,
} from './scrollAnchor.utils.ts'

// TEST-2: pure scroll-anchor math (fed synthetic rects), no DOM.

test('pickTopAnchor selects the first at-least-partially-visible box', () => {
  const viewportTop = 100
  const boxes: MessageBox[] = [
    { id: 'a', top: 0, bottom: 40 }, // fully above the fold (bottom < viewportTop)
    { id: 'b', top: 40, bottom: 120 }, // straddles the top → first visible
    { id: 'c', top: 120, bottom: 300 },
  ]
  const anchor = pickTopAnchor(boxes, viewportTop)
  assert.deepEqual(anchor, { anchorId: 'b', savedTop: 40 - 100 })
})

test('pickTopAnchor returns a fully-below box with a positive savedTop', () => {
  const anchor = pickTopAnchor(
    [{ id: 'x', top: 250, bottom: 400 }],
    100,
  )
  assert.deepEqual(anchor, { anchorId: 'x', savedTop: 150 })
})

test('pickTopAnchor returns null when nothing qualifies', () => {
  assert.equal(
    pickTopAnchor([{ id: 'a', top: 0, bottom: 50 }], 100),
    null,
  )
  assert.equal(pickTopAnchor([], 0), null)
})

test('pickTopAnchor boundary: bottom exactly at viewportTop is NOT visible (strict >)', () => {
  // bottom === viewportTop → the box ends exactly at the fold → skip it, take
  // the next one.
  const anchor = pickTopAnchor(
    [
      { id: 'a', top: 50, bottom: 100 }, // bottom == viewportTop → excluded
      { id: 'b', top: 100, bottom: 200 }, // bottom > viewportTop → chosen
    ],
    100,
  )
  assert.deepEqual(anchor, { anchorId: 'b', savedTop: 0 })
})

test('restoreDelta pins the anchor back to its saved offset', () => {
  // Content grew above by 500px: the anchor that was 40px below the viewport
  // top is now 540px below → scroll DOWN by 500 to re-pin it.
  assert.equal(restoreDelta(40, 540), 500)
  // No change → no scroll.
  assert.equal(restoreDelta(40, 40), 0)
  // Anchor moved up (rare) → negative delta scrolls up.
  assert.equal(restoreDelta(120, 80), -40)
})

// TEST-2 (virtualize): index-based restore offset for the virtualizer.
test('indexRestoreOffset re-pins the anchor index at its captured offset', () => {
  // Anchor row now starts at content-y 900; it was 120px below the viewport top
  // → scroll so 900 sits at 120 → offset 780.
  assert.equal(indexRestoreOffset(900, 120), 780)
  // Anchor straddled the fold (viewportOffset negative) → offset larger.
  assert.equal(indexRestoreOffset(900, -30), 930)
  // Clamp at the top: never scroll above 0.
  assert.equal(indexRestoreOffset(50, 200), 0)
})

// TEST-5 (message-scroll-perf ITEM-6): idempotency guard so the explicit
// prepend anchor-restore doesn't double-adjust on top of the virtualizer's own
// above-viewport size-change correction.
test('anchorRestoreNeeded skips a restore already pinned within tolerance', () => {
  // Virtualizer already put the anchor at (near) the target → no explicit scroll.
  assert.equal(anchorRestoreNeeded(780, 780), false)
  assert.equal(anchorRestoreNeeded(781, 780), false) // within default 2px
  assert.equal(anchorRestoreNeeded(778, 780), false)
})

test('anchorRestoreNeeded restores when the offset is still off target', () => {
  assert.equal(anchorRestoreNeeded(600, 780), true)
  assert.equal(anchorRestoreNeeded(783, 780), true) // beyond 2px
})

test('anchorRestoreNeeded is idempotent: after a restore to target it is a no-op', () => {
  const target = indexRestoreOffset(900, 120) // 780
  // First application: off-target → needs restore.
  assert.equal(anchorRestoreNeeded(500, target), true)
  // After scrolling to target, a second pass is a no-op (no double-count).
  assert.equal(anchorRestoreNeeded(target, target), false)
})

test('anchorRestoreNeeded honors a custom tolerance', () => {
  assert.equal(anchorRestoreNeeded(790, 780, 5), true)
  assert.equal(anchorRestoreNeeded(783, 780, 5), false)
})
