import type { MessageContent, MessageContentDataToolResult } from '@/api-client/types'

/** A single retrieved passage (verbatim from the search_knowledge tool's
 *  `structuredContent`). */
export interface KbHit {
  file_id: string
  filename: string
  page: number
  char_start: number
  char_end: number
  score: number
  content: string
}

export interface SearchKnowledgeResult {
  hits: KbHit[]
  query: string
  mode: string
  truncated: boolean
  indexing_incomplete?: { searchable: number; total: number }
}

/** The registry co-ownership predicate: claim ONLY `search_knowledge`
 *  tool_result blocks (so literature/file catch-alls still run for theirs). */
export function isSearchKnowledgeResult(content: MessageContent): boolean {
  if (content.content_type !== 'tool_result') return false
  return (content.content as MessageContentDataToolResult).name === 'search_knowledge'
}

/** Parse a tool_result block's structuredContent into a typed result, or null
 *  if it isn't a well-formed search_knowledge payload. */
export function parseSearchKnowledge(
  block: MessageContentDataToolResult,
): SearchKnowledgeResult | null {
  const sc = block.structured_content as SearchKnowledgeResult | null | undefined
  if (!sc || !Array.isArray(sc.hits)) return null
  return sc
}

/** True when the corpus was not fully indexed at query time (drives the banner). */
export function isIndexingIncomplete(sc: SearchKnowledgeResult): boolean {
  return (
    !!sc.indexing_incomplete &&
    sc.indexing_incomplete.searchable < sc.indexing_incomplete.total
  )
}

/** The serializable `kb_source` panel payload for a hit (opens the cited page). */
export function hitToPanelData(h: KbHit): {
  fileId: string
  filename: string
  page: number
  charStart: number
  charEnd: number
} {
  return {
    fileId: h.file_id,
    filename: h.filename,
    page: h.page,
    charStart: h.char_start,
    charEnd: h.char_end,
  }
}
