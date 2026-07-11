import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { KnowledgeBase, KnowledgeBaseDocument } from '@/api-client/types'
import {
  docStatusBadge,
  docToFileEntity,
  isRetryable,
  partitionKbUploads,
  summarizeIndexing,
} from './docStatus.ts'

// TEST-58 (FB-10): the upload partition itemizes rejects (which file + why),
// so the panel never shows a vague "some files failed".
test('partitionKbUploads itemizes oversize + unsupported rejects', () => {
  const accept = new Set(['pdf', 'txt', 'md'])
  const max = 1000
  const { accepted, rejected } = partitionKbUploads(
    [
      { name: 'a.pdf', size: 500 },
      { name: 'big.pdf', size: 5000 },
      { name: 'evil.exe', size: 10 },
      { name: 'notes.md', size: 1 },
    ],
    max,
    accept,
  )
  assert.deepEqual(accepted.map(f => f.name), ['a.pdf', 'notes.md'])
  assert.deepEqual(rejected, [
    { name: 'big.pdf', reason: 'too-large' },
    { name: 'evil.exe', reason: 'unsupported-type' },
  ])
})

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

// TEST-52 (FB-3, ITEM-31/17): the KB document → FileCard `File` adapter maps the
// thumbnail + subtitle fields FileCard reads (id/filename/size/mime/thumbnail),
// so the KB panel reuses the same FileCard row the project files panel uses.
test('docToFileEntity maps the FileCard-relevant fields', () => {
  const doc = {
    file_id: 'f-1',
    filename: 'protocol.pdf',
    added_at: '2026-07-10T00:00:00Z',
    index_status: 'indexed',
    chunk_count: 12,
    file_size: 2048,
    mime_type: 'application/pdf',
    has_thumbnail: true,
    preview_page_count: 3,
  } as KnowledgeBaseDocument
  const f = docToFileEntity(doc)
  assert.equal(f.id, 'f-1')
  assert.equal(f.filename, 'protocol.pdf')
  assert.equal(f.file_size, 2048)
  assert.equal(f.mime_type, 'application/pdf')
  assert.equal(f.has_thumbnail, true)
  assert.equal(f.preview_page_count, 3)
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
