import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  appendWindow,
  finalizeTailWindow,
  firstMessageId,
  indexOfMessageId,
  lastMessageId,
  mergeTailWindow,
  prependWindow,
  resumeOrFreshPlaceholder,
  toOrderedMap,
} from './messageWindow.ts'
import type { MessageWithContent } from '@/api-client/types'

function msg(
  id: string,
  text = id,
  role: 'user' | 'assistant' = 'user',
): MessageWithContent {
  return {
    id,
    role,
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

// TEST-1: finalizeTailWindow — the atomic streaming→persisted swap yields
// exactly ONE assistant row for the finished turn and never an empty window.

test('finalizeTailWindow drops a SYNTHETIC placeholder id and appends the persisted row', () => {
  // Streaming keyed the row under a synthetic `streaming-<ts>` id (no message_id
  // from the backend). The persisted tail carries the REAL id instead.
  const existing = toOrderedMap([
    msg('u1', 'hi', 'user'),
    msg('streaming-123', '', 'assistant'), // live placeholder, empty
  ])
  const tail = [msg('u1', 'hi', 'user'), msg('a1', 'the answer', 'assistant')]
  const next = finalizeTailWindow(existing, tail, 'streaming-123')
  // stale placeholder gone; persisted assistant present exactly once, at the tail
  assert.deepEqual(ids(next), ['u1', 'a1'])
  assert.equal(
    (next.get('a1')!.contents[0].content as { text: string }).text,
    'the answer',
  )
})

test('finalizeTailWindow with a REAL streaming id collapses to one in-place row', () => {
  // Backend sent message_id on content frames, so the streaming row already had
  // the real id; the persisted tail carries the same id with final content.
  const existing = toOrderedMap([
    msg('u1', 'hi', 'user'),
    msg('a1', 'partial…', 'assistant'),
  ])
  const tail = [msg('u1', 'hi', 'user'), msg('a1', 'final answer', 'assistant')]
  const next = finalizeTailWindow(existing, tail, 'a1')
  // no duplicate: one 'a1', updated content, order stable
  assert.deepEqual(ids(next), ['u1', 'a1'])
  assert.equal(
    (next.get('a1')!.contents[0].content as { text: string }).text,
    'final answer',
  )
})

test('finalizeTailWindow preserves already-loaded older pages', () => {
  // User scrolled up (older a,b,c loaded) before the streamed turn (row 's') lands.
  const existing = toOrderedMap([
    msg('a'),
    msg('b'),
    msg('c'),
    msg('s', '', 'assistant'),
  ])
  const tail = [msg('c'), msg('d', 'user turn', 'user'), msg('e', 'answer', 'assistant')]
  const next = finalizeTailWindow(existing, tail, 's')
  assert.deepEqual(ids(next), ['a', 'b', 'c', 'd', 'e'])
})

test('finalizeTailWindow with a null streamingId is a plain tail merge (no drop)', () => {
  const existing = toOrderedMap([msg('u1', 'hi', 'user')])
  const tail = [msg('u1', 'hi', 'user'), msg('a1', 'answer', 'assistant')]
  const next = finalizeTailWindow(existing, tail, null)
  assert.deepEqual(ids(next), ['u1', 'a1'])
})

// TEST-6: resumeOrFreshPlaceholder — a tool-approval resume must REUSE the
// existing assistant row (keep its content) rather than blank it with a fresh
// empty placeholder (which vanishes the bubble mid-turn).

test('resumeOrFreshPlaceholder REUSES an existing assistant row (keeps its content)', () => {
  const existing = msg('a1', 'partial answer + a tool call', 'assistant')
  const fresh = { ...msg('a1', '', 'assistant'), contents: [] } as MessageWithContent
  const chosen = resumeOrFreshPlaceholder(existing, fresh)
  assert.equal(chosen, existing) // same object → content preserved
  assert.ok(chosen.contents.length > 0)
})

test('resumeOrFreshPlaceholder uses the FRESH placeholder for a genuinely-new turn', () => {
  const fresh = { ...msg('a2', '', 'assistant'), contents: [] } as MessageWithContent
  assert.equal(resumeOrFreshPlaceholder(undefined, fresh), fresh)
})

test('resumeOrFreshPlaceholder does NOT adopt a non-assistant row for the id', () => {
  // Defensive: ids are role-unique, but never stream assistant content into a user row.
  const userRow = msg('x', 'hi', 'user')
  const fresh = { ...msg('x', '', 'assistant'), contents: [] } as MessageWithContent
  assert.equal(resumeOrFreshPlaceholder(userRow, fresh), fresh)
})

// TEST-1 (virtualize): id → array index mapping behind scrollToMessageId (the
// array is the virtualizer's item order, as MessageList calls it).
test('indexOfMessageId returns the window index or -1 when unloaded', () => {
  const arr = [msg('a'), msg('b'), msg('c')]
  assert.equal(indexOfMessageId(arr, 'a'), 0)
  assert.equal(indexOfMessageId(arr, 'b'), 1)
  assert.equal(indexOfMessageId(arr, 'c'), 2)
  assert.equal(indexOfMessageId(arr, 'missing'), -1)
  assert.equal(indexOfMessageId([], 'a'), -1)
})
