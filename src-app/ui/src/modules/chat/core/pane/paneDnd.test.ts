import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  CONVERSATION_DND_TYPE,
  PANE_DND_TYPE,
  dragKind,
  isWorkspaceDrag,
  reorderIndices,
} from './paneDnd.ts'

// TEST (split-chat ITEM-31): the drag classification that keeps a conversation/
// pane drop-zone from ever swallowing an OS file drop (the composer's job) — the
// disambiguation TEST-28 depends on. `dragKind` reads only `types` (all that is
// available during `dragover`).

const dt = (types: string[]): Pick<DataTransfer, 'types'> =>
  ({ types } as unknown as Pick<DataTransfer, 'types'>)

test('a file drag is classified as file → workspace drop-zones ignore it', () => {
  assert.equal(dragKind(dt(['Files'])), 'file')
  assert.equal(isWorkspaceDrag(dt(['Files'])), false)
})

test('a conversation drag is a workspace drag', () => {
  assert.equal(dragKind(dt([CONVERSATION_DND_TYPE])), 'conversation')
  assert.equal(isWorkspaceDrag(dt([CONVERSATION_DND_TYPE])), true)
})

test('a pane drag is a workspace drag', () => {
  assert.equal(dragKind(dt([PANE_DND_TYPE])), 'pane')
  assert.equal(isWorkspaceDrag(dt([PANE_DND_TYPE])), true)
})

test('Files wins even if a conversation type is somehow also present (no cross-fire)', () => {
  assert.equal(dragKind(dt(['Files', CONVERSATION_DND_TYPE])), 'file')
  assert.equal(isWorkspaceDrag(dt(['Files', CONVERSATION_DND_TYPE])), false)
})

test('an unrelated/plain-text drag is neither', () => {
  assert.equal(dragKind(dt(['text/plain'])), null)
  assert.equal(isWorkspaceDrag(dt(['text/plain'])), false)
})

test('reorderIndices resolves the index pair; unknown/identical → null', () => {
  const panes = [{ paneId: 'a' }, { paneId: 'b' }, { paneId: 'c' }]
  assert.deepEqual(reorderIndices(panes, 'a', 'c'), { from: 0, to: 2 })
  assert.deepEqual(reorderIndices(panes, 'c', 'a'), { from: 2, to: 0 })
  assert.equal(reorderIndices(panes, 'a', 'a'), null, 'identical is a no-op')
  assert.equal(reorderIndices(panes, 'a', 'z'), null, 'unknown target is null')
})
