import type {
  File as FileEntity,
  KnowledgeBase,
  KnowledgeBaseDocument,
} from '@/api-client/types'

export type StatusTone = 'success' | 'warning' | 'error' | 'default'

export interface KbUploadReject {
  name: string
  reason: 'too-large' | 'unsupported-type'
}

/**
 * Partition an upload batch into accepted files vs an ITEMIZED reject list
 * (which file, and why) — so the KB panel can tell the user exactly what was
 * skipped instead of a vague "some files failed". Pure, so it's unit-testable.
 */
export function partitionKbUploads<T extends { name: string; size: number }>(
  files: T[],
  maxSize: number,
  acceptedExt: Set<string>,
): { accepted: T[]; rejected: KbUploadReject[] } {
  const accepted: T[] = []
  const rejected: KbUploadReject[] = []
  for (const f of files) {
    const ext = f.name.split('.').pop()?.toLowerCase() ?? ''
    if (f.size > maxSize) rejected.push({ name: f.name, reason: 'too-large' })
    else if (!acceptedExt.has(ext)) rejected.push({ name: f.name, reason: 'unsupported-type' })
    else accepted.push(f)
  }
  return { accepted, rejected }
}

/**
 * Adapt a KB document to the `File` shape `FileCard` renders, so the KB
 * documents panel reuses the SAME FileCard row (thumbnail + size/type
 * subtitle) as the project knowledge-files panel — instead of a hand-rolled
 * list row. Only the fields FileCard reads (id/filename/size/mime/thumbnail)
 * are meaningful; the rest are inert defaults (the documents endpoint doesn't
 * carry version provenance, which FileCard never touches).
 */
export function docToFileEntity(doc: KnowledgeBaseDocument): FileEntity {
  return {
    id: doc.file_id,
    filename: doc.filename,
    file_size: doc.file_size,
    mime_type: doc.mime_type,
    has_thumbnail: doc.has_thumbnail,
    preview_page_count: doc.preview_page_count,
    // Inert defaults — not read by FileCard's row rendering.
    user_id: '',
    text_page_count: 0,
    processing_metadata: {},
    created_by: '',
    created_at: doc.added_at,
    updated_at: doc.added_at,
    version: 1,
    current_version_id: doc.file_id,
    blob_version_id: doc.file_id,
  }
}

/** Per-document index-status → badge {tone,label}. `pending` is the fallback for
 *  an unknown/absent status (a doc with no file_index_state row yet). */
export const DOC_STATUS: Record<string, { tone: StatusTone; label: string }> = {
  indexed: { tone: 'success', label: 'Indexed' },
  indexing: { tone: 'warning', label: 'Indexing' },
  pending: { tone: 'warning', label: 'Pending' },
  failed: { tone: 'error', label: 'Failed' },
  no_text: { tone: 'default', label: 'No text' },
}

export function docStatusBadge(status: string): { tone: StatusTone; label: string } {
  return DOC_STATUS[status] ?? DOC_STATUS.pending
}

/** Whether a per-document row should offer a Retry action (only terminal
 *  failure states are retryable → indexed once text is present). */
export function isRetryable(status: string): boolean {
  return status === 'failed' || status === 'no_text'
}

/** One-line indexing summary for a KB card ("M of N indexed", flagging failures /
 *  no-text). Pure projection of the KB's `indexing_summary` rollup. */
export function summarizeIndexing(kb: Pick<KnowledgeBase, 'indexing_summary'>): string {
  const s = kb.indexing_summary
  if (s.total === 0) return 'No documents'
  const parts = [`${s.indexed} of ${s.total} indexed`]
  if (s.indexing > 0) parts.push(`${s.indexing} indexing`)
  if (s.failed > 0) parts.push(`${s.failed} failed`)
  if (s.no_text > 0) parts.push(`${s.no_text} no-text`)
  return parts.join(' · ')
}
