import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  appendWindow,
  firstMessageId,
  indexOfMessageId,
  lastMessageId,
  mergeTailWindow,
  prependWindow,
  toOrderedMap,
} from './messageWindow.ts'
import type { MessageWithContent } from '@/api-client/types'

function msg(id: string, text = id): MessageWithContent {
  return {
    id,
    role: 'user',
    contents: [
      {
        id: `${id}-c0`,
        message_id: id,
        content_type: 'text',
        content: { type: 'text', text },
        sequence_order: 0,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ],
    originated_from_id: '',
    edit_count: 0,
    created_at: new Date().toISOString(),
  } as unknown as MessageWithContent
}

const ids = (m: Map<string, MessageWithContent>) => [...m.keys()]

// TEST-3: pure window-merge helpers keep render order + de-dup.

test('toOrderedMap preserves chronological order', () => {
  const m = toOrderedMap([msg('a'), msg('b'), msg('c')])
  assert.deepEqual(ids(m), ['a', 'b', 'c'])
})

test('prependWindow puts older page in front, keeping existing order', () => {
  const existing = toOrderedMap([msg('c'), msg('d')])
  const next = prependWindow(existing, [msg('a'), msg('b')])
  assert.deepEqual(ids(next), ['a', 'b', 'c', 'd'])
})

test('prependWindow drops ids already present (no dup / no reorder)', () => {
  const existing = toOrderedMap([msg('b'), msg('c')])
  // older page overlaps 'b' — the existing 'b' keeps its position.
  const next = prependWindow(existing, [msg('a'), msg('b')])
  assert.deepEqual(ids(next), ['a', 'b', 'c'])
})

test('appendWindow appends newer page and upserts overlaps in place', () => {
  const existing = toOrderedMap([msg('a'), msg('b')])
  const next = appendWindow(existing, [msg('b', 'updated'), msg('c')])
  assert.deepEqual(ids(next), ['a', 'b', 'c'])
  // overlapping 'b' updated in place, keeps its position.
  assert.equal(
    (next.get('b')!.contents[0].content as { text: string }).text,
    'updated',
  )
})

test('mergeTailWindow keeps loaded older pages and appends the new tail', () => {
  // Simulate a user who scrolled up (a,b,c loaded) then a new turn (d,e) lands.
  const existing = toOrderedMap([msg('a'), msg('b'), msg('c')])
  const next = mergeTailWindow(existing, [msg('c'), msg('d'), msg('e')])
  assert.deepEqual(ids(next), ['a', 'b', 'c', 'd', 'e'])
})

test('empty page inputs are no-ops that preserve order + identity', () => {
  const existing = toOrderedMap([msg('a'), msg('b')])
  assert.deepEqual(ids(prependWindow(existing, [])), ['a', 'b'])
  assert.deepEqual(ids(appendWindow(existing, [])), ['a', 'b'])
  assert.deepEqual(ids(mergeTailWindow(existing, [])), ['a', 'b'])
  // prepend/append onto an empty window just adopt the page.
  const empty = new Map<string, MessageWithContent>()
  assert.deepEqual(ids(prependWindow(empty, [msg('x')])), ['x'])
  assert.deepEqual(ids(appendWindow(empty, [msg('y')])), ['y'])
})

test('firstMessageId / lastMessageId read the window boundaries', () => {
  const m = toOrderedMap([msg('a'), msg('b'), msg('c')])
  assert.equal(firstMessageId(m), 'a')
  assert.equal(lastMessageId(m), 'c')
  const empty = new Map<string, MessageWithContent>()
  assert.equal(firstMessageId(empty), null)
  assert.equal(lastMessageId(empty), null)
})

// TEST-1 (virtualize): id → window index mapping behind scrollToMessageId.
test('indexOfMessageId returns the window index or -1 when unloaded', () => {
  const m = toOrderedMap([msg('a'), msg('b'), msg('c')])
  assert.equal(indexOfMessageId(m, 'a'), 0)
  assert.equal(indexOfMessageId(m, 'b'), 1)
  assert.equal(indexOfMessageId(m, 'c'), 2)
  assert.equal(indexOfMessageId(m, 'missing'), -1)
  assert.equal(indexOfMessageId(new Map<string, MessageWithContent>(), 'a'), -1)
})
