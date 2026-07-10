import type { KnowledgeBase } from '@/api-client/types'

export type StatusTone = 'success' | 'warning' | 'error' | 'default'

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
