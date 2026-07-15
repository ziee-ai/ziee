import { test } from 'node:test'
import assert from 'node:assert/strict'
import { PENDING_KB_KEY, kbKey, pendingKbKey, selectedKbIdsFor } from './kbSelectionKey.ts'

// TEST-70 (ITEM-46): the per-conversation KB selection keying. Two split panes on
// different conversations must show + mutate their OWN selection, never a shared
// one; a new chat uses the pending key.
// TEST-76 (ITEM-51): the PENDING (pre-mint new-chat) buffer is keyed per PANE, so
// two split panes each composing a NEW chat don't share one buffer either.

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

test('pendingKbKey: per-pane pending key; null pane → the bare key (single-pane unchanged)', () => {
  assert.equal(pendingKbKey(null), PENDING_KB_KEY)
  assert.equal(pendingKbKey(undefined), PENDING_KB_KEY)
  assert.equal(pendingKbKey(''), PENDING_KB_KEY, 'empty pane id → single-pane bare key')
  assert.equal(pendingKbKey('pane-A'), `${PENDING_KB_KEY}:pane-A`)
  assert.notEqual(
    pendingKbKey('pane-A'),
    pendingKbKey('pane-B'),
    'two panes composing new chats get distinct pending keys',
  )
})

test('kbKey/selectedKbIdsFor: two new-chat panes read their OWN pending buffer (ITEM-51)', () => {
  // Both panes are NEW chats (conversationId null) but keyed by distinct pane ids.
  const map = new Map<string, Set<string>>([
    [pendingKbKey('pane-A'), new Set(['kbA'])],
    [pendingKbKey('pane-B'), new Set(['kbB'])],
  ])
  assert.equal(kbKey(null, 'pane-A'), `${PENDING_KB_KEY}:pane-A`)
  assert.deepEqual(selectedKbIdsFor(map, null, 'pane-A'), ['kbA'])
  assert.deepEqual(
    selectedKbIdsFor(map, null, 'pane-B'),
    ['kbB'],
    'pane B\'s pending selection is its OWN — pane A\'s kbA does not leak in',
  )
  // A real conversation id still ignores paneId (committed state keys by conv id).
  assert.equal(kbKey('conv-Z', 'pane-A'), 'conv-Z')
})
