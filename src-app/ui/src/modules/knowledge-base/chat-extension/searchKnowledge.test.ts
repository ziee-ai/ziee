import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { MessageContent, MessageContentDataToolResult } from '@/api-client/types'
import {
  hitToPanelData,
  isIndexingIncomplete,
  isSearchKnowledgeResult,
  parseSearchKnowledge,
  type SearchKnowledgeResult,
} from './searchKnowledge.ts'

// TEST-36 (ITEM-35,36,37): the search_knowledge parsing/co-ownership helpers the
// transparency card is built on (drift: the citation surface is the card, not
// `[n]` chips — the tool cites by file/page in prose).

function toolResult(name: string, structured?: unknown): MessageContent {
  return {
    content_type: 'tool_result',
    content: {
      tool_use_id: 'tu1',
      name,
      content: '',
      structured_content: structured,
      is_error: false,
    } as unknown as MessageContentDataToolResult,
  } as unknown as MessageContent
}

test('isSearchKnowledgeResult claims only search_knowledge tool_results', () => {
  assert.equal(isSearchKnowledgeResult(toolResult('search_knowledge', { hits: [] })), true)
  assert.equal(isSearchKnowledgeResult(toolResult('literature_search', {})), false)
  assert.equal(
    isSearchKnowledgeResult({ content_type: 'text', content: { text: 'x' } } as unknown as MessageContent),
    false,
  )
})

test('parseSearchKnowledge returns null unless hits is an array', () => {
  const block = (toolResult('search_knowledge', { query: 'q', hits: [{ file_id: 'f' }] }) as { content: MessageContentDataToolResult }).content
  assert.ok(parseSearchKnowledge(block))
  const bad = (toolResult('search_knowledge', { query: 'q' }) as { content: MessageContentDataToolResult }).content
  assert.equal(parseSearchKnowledge(bad), null)
  const nullish = (toolResult('search_knowledge', null) as { content: MessageContentDataToolResult }).content
  assert.equal(parseSearchKnowledge(nullish), null)
})

test('isIndexingIncomplete flips only when searchable < total', () => {
  const base = { hits: [], query: 'q', mode: 'HYBRID', truncated: false }
  assert.equal(isIndexingIncomplete({ ...base, indexing_incomplete: { searchable: 3, total: 5 } } as SearchKnowledgeResult), true)
  assert.equal(isIndexingIncomplete({ ...base, indexing_incomplete: { searchable: 5, total: 5 } } as SearchKnowledgeResult), false)
  assert.equal(isIndexingIncomplete(base as SearchKnowledgeResult), false)
})

test('hitToPanelData builds the serializable kb_source payload', () => {
  const data = hitToPanelData({
    file_id: 'f1', filename: 'doc.pdf', page: 3, char_start: 10, char_end: 42, score: 0.9, content: '…',
  })
  assert.deepEqual(data, { fileId: 'f1', filename: 'doc.pdf', page: 3, charStart: 10, charEnd: 42 })
})
