import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import { useSplitViewStore } from './splitView'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'

// TEST-1 / TEST-2 (split-chat ITEM-1): the SplitView LAYOUT store — pane
// open/close/focus/reorder/divider mutations + the localStorage-shape state.
// Driven headless via getState() actions (no React / no window routing — URL
// mirroring was dropped per DRIFT-1.9, so persistence is the in-memory + storage
// layout shape, not `?pane=`).

const s = () => useSplitViewStore.getState()

beforeEach(() => {
  void s().reset()
})

test('openPane appends + focuses; returns the new paneId', async () => {
  const id1 = await s().openPane({ conversationId: 'c1' })
  assert.ok(id1, 'openPane returns a paneId')
  assert.equal(s().panes.length, 1)
  assert.equal(s().panes[0].conversationId, 'c1')
  assert.equal(s().focusedPaneId, id1, 'the newly opened pane is focused')

  const id2 = await s().openPane({ conversationId: 'c2' })
  assert.equal(s().panes.length, 2)
  assert.equal(s().focusedPaneId, id2, 'focus moves to the latest opened pane')
})

test('openPane caps at MAX_PANES and returns null past the cap', async () => {
  const ids: (string | null)[] = []
  for (let i = 0; i < SPLIT_LIMITS.MAX_PANES; i++) {
    ids.push(await s().openPane({ conversationId: `c${i}` }))
  }
  assert.equal(s().panes.length, SPLIT_LIMITS.MAX_PANES)
  assert.ok(ids.every(Boolean), 'every pane up to the cap opens')
  const over = await s().openPane({ conversationId: 'overflow' })
  assert.equal(over, null, 'opening past MAX_PANES returns null')
  assert.equal(s().panes.length, SPLIT_LIMITS.MAX_PANES, 'no pane added past the cap')
})

test('openPane with afterPaneId inserts after that pane', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const c = await s().openPane({ conversationId: 'c' })
  const b = await s().openPane({ conversationId: 'b', afterPaneId: a! })
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['a', 'b', 'c'],
    'the new pane is spliced directly after afterPaneId',
  )
  assert.equal(s().focusedPaneId, b)
  assert.ok(c)
})

test('openPane beforePaneId inserts before a MIDDLE pane (ITEM-70 insert-left)', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  const z = await s().openPane({ conversationId: 'z', beforePaneId: b! })
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['a', 'z', 'b'],
    'the new pane is spliced directly BEFORE beforePaneId',
  )
  assert.equal(s().focusedPaneId, z)
  assert.ok(a)
})

test('openPane beforePaneId before the FIRST pane prepends', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  const z = await s().openPane({ conversationId: 'z', beforePaneId: a! })
  assert.deepEqual(s().panes.map((p) => p.conversationId), ['z', 'a', 'b'])
  assert.equal(s().focusedPaneId, z)
  assert.ok(b)
})

test('closePane atomically reassigns focus to a surviving neighbour', async () => {
  await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  const c = await s().openPane({ conversationId: 'c' })
  await s().focusPane(b!)
  await s().closePane(b!)
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

test('closing the last pane sets focus to null', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  await s().closePane(a!)
  assert.equal(s().panes.length, 0)
  assert.equal(s().focusedPaneId, null)
})

test('focusPane only focuses an existing pane', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  await s().focusPane('does-not-exist')
  assert.equal(s().focusedPaneId, a, 'focusing an unknown pane is a no-op')
  await s().focusPane(a!)
  assert.equal(s().focusedPaneId, a)
})

test('setPaneConversation points a pane at a (different) conversation', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  await s().setPaneConversation(a!, 'a2', 'proj1')
  const pane = s().panes.find((p) => p.paneId === a)!
  assert.equal(pane.conversationId, 'a2')
  assert.equal(pane.projectId, 'proj1')
})

test('reorderPanes moves a pane; out-of-bounds is a no-op', async () => {
  await s().openPane({ conversationId: 'a' })
  await s().openPane({ conversationId: 'b' })
  await s().openPane({ conversationId: 'c' })
  await s().reorderPanes(0, 2)
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
  )
  await s().reorderPanes(5, 0) // fromIndex out of bounds
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
    'an out-of-bounds fromIndex leaves the order unchanged',
  )
  await s().reorderPanes(0, 9) // toIndex out of bounds (guards both ends — audit LOW)
  assert.deepEqual(
    s().panes.map((p) => p.conversationId),
    ['b', 'c', 'a'],
    'an out-of-bounds toIndex leaves the order unchanged',
  )
})

// TEST-46 (split-chat ITEM-24): the one-conversation-per-workspace guard on the
// low-level actions + paneId stability across reorder.
test('openPane for a conversation already open focuses that pane, no duplicate', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  const again = await s().openPane({ conversationId: 'a' })
  assert.equal(again, a, 'openPane returns the existing pane holding that conversation')
  assert.equal(s().panes.length, 2, 'no duplicate pane is created')
  assert.equal(s().focusedPaneId, a, 'the existing pane is focused')
  assert.ok(b)
})

test('setPaneConversation onto a conversation open elsewhere focuses it, no duplicate', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  // Point pane b at conversation 'a' (already in pane a) — must NOT duplicate.
  await s().setPaneConversation(b!, 'a')
  assert.equal(s().panes.length, 2)
  assert.equal(
    s().panes.filter((p) => p.conversationId === 'a').length,
    1,
    'conversation a is still held by exactly one pane',
  )
  assert.equal(s().focusedPaneId, a, 'focus moves to the pane already holding a')
})

test('paneId + conversation are stable across reorderPanes', async () => {
  const a = await s().openPane({ conversationId: 'a' })
  const b = await s().openPane({ conversationId: 'b' })
  const c = await s().openPane({ conversationId: 'c' })
  await s().reorderPanes(0, 2)
  const ids = s().panes.map((p) => p.paneId)
  const convs = s().panes.map((p) => p.conversationId)
  assert.deepEqual(convs, ['b', 'c', 'a'], 'conversations move with their panes')
  assert.deepEqual(ids, [b, c, a], 'each pane keeps its stable id when moved')
})

test('setDividerWidth clamps to MIN/MAX_PANE_WIDTH', async () => {
  await s().setDividerWidth(0, 10) // below MIN
  assert.equal(s().dividerWidths[0], SPLIT_LIMITS.MIN_PANE_WIDTH)
  await s().setDividerWidth(0, 99999) // above MAX
  assert.equal(s().dividerWidths[0], SPLIT_LIMITS.MAX_PANE_WIDTH)
  const mid = Math.round(
    (SPLIT_LIMITS.MIN_PANE_WIDTH + SPLIT_LIMITS.MAX_PANE_WIDTH) / 2,
  )
  await s().setDividerWidth(0, mid)
  assert.equal(s().dividerWidths[0], mid)
})

test('setMode toggles split/tabs; reset clears the layout', async () => {
  await s().openPane({ conversationId: 'a' })
  await s().setMode('tabs')
  assert.equal(s().mode, 'tabs')
  await s().reset()
  assert.equal(s().panes.length, 0)
  assert.equal(s().focusedPaneId, null)
  assert.deepEqual(s().dividerWidths, [])
  assert.equal(s().mode, 'split')
})

// TEST-117 (ITEM-83 / FB-26): the small-screen pane-manager Drawer open-state is a
// TRANSIENT store field — `setPaneManagerOpen` toggles it, it defaults closed, and
// toggling it must NOT perturb any of the fields that get persisted (panes /
// focusedPaneId / dividerWidths / mode / direction — the `snapshot()` set). Proving
// orthogonality here is the unit-level guard that the drawer state is never saved.
test('setPaneManagerOpen toggles paneManagerOpen without touching the persisted layout', async () => {
  await s().openPane({ conversationId: 'c1' })
  await s().openPane({ conversationId: 'c2' })
  const persistedFields = () =>
    JSON.stringify({
      panes: s().panes,
      focusedPaneId: s().focusedPaneId,
      dividerWidths: s().dividerWidths,
      mode: s().mode,
      direction: s().direction,
    })
  const before = persistedFields()
  assert.equal(s().paneManagerOpen, false, 'defaults closed')
  await s().setPaneManagerOpen(true)
  assert.equal(s().paneManagerOpen, true, 'opens')
  await s().setPaneManagerOpen(false)
  assert.equal(s().paneManagerOpen, false, 'closes')
  assert.equal(
    persistedFields(),
    before,
    'toggling paneManagerOpen leaves every persisted layout field unchanged (transient)',
  )
})
