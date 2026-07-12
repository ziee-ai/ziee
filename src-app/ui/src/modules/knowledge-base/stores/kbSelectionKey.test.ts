import { test } from 'node:test'
import assert from 'node:assert/strict'
import { PENDING_KB_KEY, kbKey, selectedKbIdsFor } from './kbSelectionKey.ts'

// TEST-70 (ITEM-46): the per-conversation KB selection keying. Two split panes on
// different conversations must show + mutate their OWN selection, never a shared
// one; a new chat uses the pending key.

test('kbKey: a conversation maps to itself; null/empty → the pending key', () => {
  assert.equal(kbKey('conv-A'), 'conv-A')
  assert.equal(kbKey(null), PENDING_KB_KEY)
  assert.equal(kbKey(undefined), PENDING_KB_KEY)
  assert.equal(kbKey(''), PENDING_KB_KEY)
})

test('selectedKbIdsFor: reads ONLY the queried conversation (no cross-pane leak)', () => {
  const map = new Map<string, Set<string>>([
    ['conv-A', new Set(['kb1', 'kb2'])],
    ['conv-B', new Set(['kb3'])],
  ])
  assert.deepEqual(selectedKbIdsFor(map, 'conv-A').sort(), ['kb1', 'kb2'])
  assert.deepEqual(selectedKbIdsFor(map, 'conv-B'), ['kb3'])
  assert.deepEqual(
    selectedKbIdsFor(map, 'conv-C'),
    [],
    'a conversation with no slot reads empty, not another pane\'s',
  )
})

test('selectedKbIdsFor: a new chat reads the pending slot', () => {
  const map = new Map<string, Set<string>>([[PENDING_KB_KEY, new Set(['kbX'])]])
  assert.deepEqual(selectedKbIdsFor(map, null), ['kbX'])
  assert.deepEqual(selectedKbIdsFor(map, 'conv-A'), [], 'a real conv does not read pending')
})
