import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import { useSplitViewStore } from './SplitView.store.ts'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'

// TEST-1 / TEST-2 (split-chat ITEM-1): the SplitView LAYOUT store — pane
// open/close/focus/reorder/divider mutations + the localStorage-shape state.
// Driven headless via getState() actions (no React / no window routing — URL
// mirroring was dropped per DRIFT-1.9, so persistence is the in-memory + storage
// layout shape, not `?pane=`).

const s = () => useSplitViewStore.getState()

beforeEach(() => {
  s().reset()
})

test('openPane appends + focuses; returns the new paneId', () => {
  const id1 = s().openPane({ conversationId: 'c1' })
  assert.ok(id1, 'openPane returns a paneId')
  assert.equal(s().panes.length, 1)
  assert.equal(s().panes[0].conversationId, 'c1')
  assert.equal(s().focusedPaneId, id1, 'the newly opened pane is focused')

  const id2 = s().openPane({ conversationId: 'c2' })
  assert.equal(s().panes.length, 2)
  assert.equal(s().focusedPaneId, id2, 'focus moves to the latest opened pane')
})

test('openPane caps at MAX_PANES and returns null past the cap', () => {
  const ids = []
  for (let i = 0; i < SPLIT_LIMITS.MAX_PANES; i++) {
    ids.push(s().openPane({ conversationId: `c${i}` }))
  }
  assert.equal(s().panes.length, SPLIT_LIMITS.MAX_PANES)
  assert.ok(ids.every(Boolean), 'every pane up to the cap opens')
  const over = s().openPane({ conversationId: 'overflow' })
  assert.equal(over, null, 'opening past MAX_PANES returns null')
  assert.equal(s().panes.length, SPLIT_LIMITS.MAX_PANES, 'no pane added past the cap')
})

test('openPane with afterPaneId inserts after that pane', () => {
  const a = s().openPane({ conversationId: 'a' })
  const c = s().openPane({ conversationId: 'c' })
  const b = s().openPane({ conversationId: 'b', afterPaneId: a! })
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['a', 'b', 'c'],
    'the new pane is spliced directly after afterPaneId',
  )
  assert.equal(s().focusedPaneId, b)
  assert.ok(c)
})

test('openPane beforePaneId inserts before a MIDDLE pane (ITEM-70 insert-left)', () => {
  const a = s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  const z = s().openPane({ conversationId: 'z', beforePaneId: b! })
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['a', 'z', 'b'],
    'the new pane is spliced directly BEFORE beforePaneId',
  )
  assert.equal(s().focusedPaneId, z)
  assert.ok(a)
})

test('openPane beforePaneId before the FIRST pane prepends', () => {
  const a = s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  const z = s().openPane({ conversationId: 'z', beforePaneId: a! })
  assert.deepEqual(s().panes.map((p) => p.conversationId), ['z', 'a', 'b'])
  assert.equal(s().focusedPaneId, z)
  assert.ok(b)
})

test('closePane atomically reassigns focus to a surviving neighbour', () => {
  s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  const c = s().openPane({ conversationId: 'c' })
  s().focusPane(b!)
  s().closePane(b!)
  // Focus deterministically reassigns to the pane now AT the closed index
  // (panes[idx] === c) — pinned exactly (not `a || c`) so a regression that
  // changed neighbour-selection to panes[idx-1] would fail (audit MEDIUM,
  // fork1/a6aff). Never null while panes remain, never the removed pane.
  assert.equal(s().panes.length, 2)
  assert.equal(
    s().focusedPaneId,
    c,
    'focus reassigns to the pane now at the closed index (c)',
  )
  assert.notEqual(s().focusedPaneId, b, 'focus never points at the removed pane')
  assert.ok(s().panes.every((p) => p.paneId !== b))
})

test('closing the last pane sets focus to null', () => {
  const a = s().openPane({ conversationId: 'a' })
  s().closePane(a!)
  assert.equal(s().panes.length, 0)
  assert.equal(s().focusedPaneId, null)
})

test('focusPane only focuses an existing pane', () => {
  const a = s().openPane({ conversationId: 'a' })
  s().focusPane('does-not-exist')
  assert.equal(s().focusedPaneId, a, 'focusing an unknown pane is a no-op')
  s().focusPane(a!)
  assert.equal(s().focusedPaneId, a)
})

test('setPaneConversation points a pane at a (different) conversation', () => {
  const a = s().openPane({ conversationId: 'a' })
  s().setPaneConversation(a!, 'a2', 'proj1')
  const pane = s().panes.find((p) => p.paneId === a)!
  assert.equal(pane.conversationId, 'a2')
  assert.equal(pane.projectId, 'proj1')
})

test('reorderPanes moves a pane; out-of-bounds is a no-op', () => {
  s().openPane({ conversationId: 'a' })
  s().openPane({ conversationId: 'b' })
  s().openPane({ conversationId: 'c' })
  s().reorderPanes(0, 2)
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
  )
  s().reorderPanes(5, 0) // fromIndex out of bounds
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
    'an out-of-bounds fromIndex leaves the order unchanged',
  )
  s().reorderPanes(0, 9) // toIndex out of bounds (guards both ends — audit LOW)
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
    'an out-of-bounds toIndex leaves the order unchanged',
  )
})

// TEST-46 (split-chat ITEM-24): the one-conversation-per-workspace guard on the
// low-level actions + paneId stability across reorder.
test('openPane for a conversation already open focuses that pane, no duplicate', () => {
  const a = s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  const again = s().openPane({ conversationId: 'a' })
  assert.equal(again, a, 'openPane returns the existing pane holding that conversation')
  assert.equal(s().panes.length, 2, 'no duplicate pane is created')
  assert.equal(s().focusedPaneId, a, 'the existing pane is focused')
  assert.ok(b)
})

test('setPaneConversation onto a conversation open elsewhere focuses it, no duplicate', () => {
  const a = s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  // Point pane b at conversation 'a' (already in pane a) — must NOT duplicate.
  s().setPaneConversation(b!, 'a')
  assert.equal(s().panes.length, 2)
  assert.equal(
    s().panes.filter((p) => p.conversationId === 'a').length,
    1,
    'conversation a is still held by exactly one pane',
  )
  assert.equal(s().focusedPaneId, a, 'focus moves to the pane already holding a')
})

test('paneId + conversation are stable across reorderPanes', () => {
  const a = s().openPane({ conversationId: 'a' })
  const b = s().openPane({ conversationId: 'b' })
  const c = s().openPane({ conversationId: 'c' })
  s().reorderPanes(0, 2)
  const ids = s().panes.map((p) => p.paneId)
  const convs = s().panes.map((p) => p.conversationId)
  assert.deepEqual(convs, ['b', 'c', 'a'], 'conversations move with their panes')
  assert.deepEqual(ids, [b, c, a], 'each pane keeps its stable id when moved')
})

test('setDividerWidth clamps to MIN/MAX_PANE_WIDTH', () => {
  s().setDividerWidth(0, 10) // below MIN
  assert.equal(s().dividerWidths[0], SPLIT_LIMITS.MIN_PANE_WIDTH)
  s().setDividerWidth(0, 99999) // above MAX
  assert.equal(s().dividerWidths[0], SPLIT_LIMITS.MAX_PANE_WIDTH)
  const mid = Math.round(
    (SPLIT_LIMITS.MIN_PANE_WIDTH + SPLIT_LIMITS.MAX_PANE_WIDTH) / 2,
  )
  s().setDividerWidth(0, mid)
  assert.equal(s().dividerWidths[0], mid)
})

test('setMode toggles split/tabs; reset clears the layout', () => {
  s().openPane({ conversationId: 'a' })
  s().setMode('tabs')
  assert.equal(s().mode, 'tabs')
  s().reset()
  assert.equal(s().panes.length, 0)
  assert.equal(s().focusedPaneId, null)
  assert.deepEqual(s().dividerWidths, [])
  assert.equal(s().mode, 'split')
})
