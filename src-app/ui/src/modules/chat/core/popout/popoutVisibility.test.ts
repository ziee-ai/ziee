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
