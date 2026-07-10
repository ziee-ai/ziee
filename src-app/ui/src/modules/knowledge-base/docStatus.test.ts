import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { KnowledgeBase } from '@/api-client/types'
import { docStatusBadge, isRetryable, summarizeIndexing } from './docStatus.ts'

// TEST-35 (ITEM-27,31): per-document index-status → badge, incl. no_text and the
// pending fallback for an unknown/absent status.
test('docStatusBadge maps each terminal status (no_text included)', () => {
  assert.deepEqual(docStatusBadge('indexed'), { tone: 'success', label: 'Indexed' })
  assert.deepEqual(docStatusBadge('indexing'), { tone: 'warning', label: 'Indexing' })
  assert.deepEqual(docStatusBadge('failed'), { tone: 'error', label: 'Failed' })
  assert.deepEqual(docStatusBadge('no_text'), { tone: 'default', label: 'No text' })
  // unknown / absent status → pending fallback (a doc with no file_index_state row)
  assert.deepEqual(docStatusBadge('bogus'), { tone: 'warning', label: 'Pending' })
})

test('isRetryable only for the recoverable terminal states', () => {
  assert.equal(isRetryable('failed'), true)
  assert.equal(isRetryable('no_text'), true)
  assert.equal(isRetryable('indexed'), false)
  assert.equal(isRetryable('indexing'), false)
})

// TEST-34 (ITEM-27): the KB indexing-summary rollup → a one-line display string.
function kb(summary: Partial<KnowledgeBase['indexing_summary']>): KnowledgeBase {
  return {
    indexing_summary: {
      total: 0, indexed: 0, indexing: 0, failed: 0, no_text: 0, pending: 0, ...summary,
    },
  } as unknown as KnowledgeBase
}

test('summarizeIndexing projects the rollup (empty / all-indexed / mixed)', () => {
  assert.equal(summarizeIndexing(kb({ total: 0 })), 'No documents')
  assert.equal(summarizeIndexing(kb({ total: 5, indexed: 5 })), '5 of 5 indexed')
  assert.equal(
    summarizeIndexing(kb({ total: 6, indexed: 3, indexing: 1, failed: 1, no_text: 1 })),
    '3 of 6 indexed · 1 indexing · 1 failed · 1 no-text',
  )
})
