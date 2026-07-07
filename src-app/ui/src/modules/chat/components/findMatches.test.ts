import { test } from 'node:test'
import assert from 'node:assert/strict'
import { findMatches, messageText } from './findMatches.ts'
import type { MessageWithContent } from '@/api-client/types'

function msg(id: string, texts: string[], role = 'user'): MessageWithContent {
  return {
    id,
    role,
    contents: texts.map((t, i) => ({
      id: `${id}-c${i}`,
      message_id: id,
      content_type: 'text',
      content: { type: 'text', text: t },
      sequence_order: i,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    })),
    originated_from_id: '',
    edit_count: 0,
    created_at: new Date().toISOString(),
  } as unknown as MessageWithContent
}

const conversation: MessageWithContent[] = [
  msg('m1', ['Hello there, this is about pgvector']),
  msg('m2', ['A reply mentioning Postgres and PGVECTOR again'], 'assistant'),
  msg('m3', ['nothing relevant here']),
]

test('returns matching message ids in display order, case-insensitive', () => {
  assert.deepEqual(findMatches(conversation, 'pgvector'), ['m1', 'm2'])
  assert.deepEqual(findMatches(conversation, 'PGVECTOR'), ['m1', 'm2'])
})

test('blank / whitespace query matches nothing', () => {
  assert.deepEqual(findMatches(conversation, ''), [])
  assert.deepEqual(findMatches(conversation, '   '), [])
})

test('no match returns empty', () => {
  assert.deepEqual(findMatches(conversation, 'zzz-not-present'), [])
})

test('ignores non-text content blocks', () => {
  const withFile = {
    id: 'mf',
    role: 'user',
    contents: [
      {
        id: 'mf-c0',
        message_id: 'mf',
        content_type: 'file_attachment',
        content: { type: 'file_attachment', filename: 'pgvector.pdf' },
        sequence_order: 0,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ],
    originated_from_id: '',
    edit_count: 0,
    created_at: new Date().toISOString(),
  } as unknown as MessageWithContent
  // The filename contains the term but it's not a text block → no match.
  assert.deepEqual(findMatches([withFile], 'pgvector'), [])
  assert.equal(messageText(withFile), '')
})
