import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
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
