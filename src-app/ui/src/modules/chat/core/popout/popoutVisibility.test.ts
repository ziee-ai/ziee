import { test } from 'node:test'
import assert from 'node:assert/strict'
import { popoutActionVisible } from './popoutVisibility.ts'

// TEST-65 (ITEM-44 / DEC-60 / FB-9): the pop-out action is desktop-only in
// single-pane, but always shown inside a split pane. Pure truth table.

test('popoutActionVisible: inside a split pane it always shows (both platforms)', () => {
  assert.equal(popoutActionVisible(true, false), true) // split pane, web
  assert.equal(popoutActionVisible(true, true), true) // split pane, desktop
})

test('popoutActionVisible: single-pane shows on desktop only', () => {
  assert.equal(popoutActionVisible(false, true), true) // single-pane, desktop → show
  assert.equal(
    popoutActionVisible(false, false),
    false,
    'single-pane web hides it (browser has its own new-tab)',
  )
})

// TEST-65b (ITEM-56 / FB-13): NEVER inside the pop-out WINDOW itself — even on the
// desktop split-pane cases that would otherwise show it. It's a focused
// single-conversation window; "open in new window" there is a self-focusing no-op.
test('popoutActionVisible: hidden inside the pop-out window regardless of pane/platform', () => {
  assert.equal(popoutActionVisible(false, true, true), false) // single-pane, desktop, in pop-out
  assert.equal(popoutActionVisible(true, true, true), false) // split, desktop, in pop-out
  assert.equal(popoutActionVisible(true, false, true), false) // split, web, in pop-out
})
