import { test } from 'node:test'
import assert from 'node:assert/strict'
import { planPopoutSnapBack, handlePopoutClosed } from './planPopoutSnapBack.ts'

// TEST-81 (ITEM-54): the snap-back decision — a closed pop-out window's conversation
// returns to the main window as a pane, but never duplicated and never past the cap.
// TEST-82 (ITEM-54): the handler control flow that RUNS the snap-back — on a pop-out
// close it opens the conversation back as a new pane ONLY when the decision is `add`.

test('add: a closed pop-out conversation NOT already shown → open as a new pane', () => {
  // Single-pane main window showing a DIFFERENT conversation.
  assert.equal(
    planPopoutSnapBack('c-closed', {
      paneConversationIds: [],
      singlePaneConversationId: 'c-other',
      maxPanes: 3,
    }),
    'add',
  )
  // Split main window with room.
  assert.equal(
    planPopoutSnapBack('c-closed', {
      paneConversationIds: ['a', 'b'],
      singlePaneConversationId: null,
      maxPanes: 3,
    }),
    'add',
  )
})

test('alreadyShown: never duplicate — it is in a pane, or IS the single-pane conversation', () => {
  assert.equal(
    planPopoutSnapBack('a', {
      paneConversationIds: ['a', 'b'],
      singlePaneConversationId: null,
      maxPanes: 3,
    }),
    'alreadyShown',
  )
  assert.equal(
    planPopoutSnapBack('c-solo', {
      paneConversationIds: [],
      singlePaneConversationId: 'c-solo',
      maxPanes: 3,
    }),
    'alreadyShown',
  )
})

test('atCap: main window already holds MAX_PANES → cannot snap back another', () => {
  assert.equal(
    planPopoutSnapBack('c-closed', {
      paneConversationIds: ['a', 'b', 'c'],
      singlePaneConversationId: null,
      maxPanes: 3,
    }),
    'atCap',
  )
})

// ── TEST-82: handlePopoutClosed control flow (RUNS the snap-back) ──
function harness(
  overrides: Partial<Parameters<typeof handlePopoutClosed>[1]> = {},
) {
  const opened: string[] = []
  const deps = {
    getPaneConversationIds: () => [] as Array<string | null>,
    getSinglePaneConversationId: () => null as string | null,
    maxPanes: 3,
    openAsNewPane: (id: string) => opened.push(id),
    ...overrides,
  }
  return { deps, opened }
}

test('handlePopoutClosed: add → opens the conversation back as a new pane', () => {
  const { deps, opened } = harness({ getPaneConversationIds: () => ['a', 'b'] })
  handlePopoutClosed('c-closed', deps)
  assert.deepEqual(opened, ['c-closed'])
})

test('handlePopoutClosed: alreadyShown → does NOT re-open (no duplicate pane)', () => {
  const { deps, opened } = harness({
    getPaneConversationIds: () => ['a', 'c-closed'],
  })
  handlePopoutClosed('c-closed', deps)
  assert.deepEqual(opened, [], 'already in a pane → snap-back is a no-op')
})

test('handlePopoutClosed: atCap → does NOT re-open (workspace full)', () => {
  const { deps, opened } = harness({
    getPaneConversationIds: () => ['a', 'b', 'c'],
  })
  handlePopoutClosed('c-closed', deps)
  assert.deepEqual(opened, [])
})
