import { BookOpen } from 'lucide-react'
import { Button, Card, Tag, Text } from '@/components/ui'
import type { MessageContentDataToolResult } from '@/api-client/types'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import { MessageFilesView } from '@/modules/file/chat-extension/components/MessageFilesView'

/** A single retrieved passage (verbatim from the tool's `structuredContent`). */
interface KbHit {
  file_id: string
  filename: string
  page: number
  char_start: number
  char_end: number
  score: number
  content: string
}
interface SearchKnowledgeResult {
  hits: KbHit[]
  query: string
  mode: string
  truncated: boolean
  indexing_incomplete?: { searchable: number; total: number }
}

/**
 * Inline renderer for a `search_knowledge` tool result — the retrieval
 * transparency panel. Shows the query, the fusion mode, an indexing-incomplete
 * warning, and each retrieved passage (source file · page · score) with a jump
 * to the source document.
 *
 * The content-type registry is FIRST-WINS (registry.tsx `renderContent`): the
 * file extension also claims `tool_result`, so this registers at a lower
 * `priority` and DELEGATES every non-`search_knowledge` block back to
 * MessageFilesView — mirrors LiteratureToolResultCard.
 */
export function SearchKnowledgeToolResultCard(props: ContentRendererProps) {
  const { content } = props
  if (content.content_type !== 'tool_result') return null
  const block = content.content as MessageContentDataToolResult
  if (block.name !== 'search_knowledge') return <MessageFilesView {...props} />
  const sc = block.structured_content as SearchKnowledgeResult | null | undefined
  if (!sc || !Array.isArray(sc.hits)) return <MessageFilesView {...props} />

  const incomplete =
    sc.indexing_incomplete && sc.indexing_incomplete.searchable < sc.indexing_incomplete.total

  const openSource = (h: KbHit) => {
    Stores.Chat.displayInRightPanel({
      id: `kb:${h.file_id}:${h.page}:${h.char_start}`,
      title: `${h.filename} · p${h.page}`.slice(0, 60),
      type: 'kb_source',
      data: {
        fileId: h.file_id,
        filename: h.filename,
        page: h.page,
        charStart: h.char_start,
        charEnd: h.char_end,
      },
    })
  }

  return (
    <Card size="sm" className="my-2" data-testid="kb-tool-result-card">
      <Text strong>
        <BookOpen /> Knowledge base search
      </Text>
      <Text
        type="secondary"
        className="!mb-2 text-xs block"
        data-testid="kb-tool-result-summary"
      >
        “{sc.query}” — {sc.hits.length} passage{sc.hits.length === 1 ? '' : 's'} ·{' '}
        {sc.mode.toLowerCase()}
        {sc.truncated ? ' · truncated' : ''}
        {incomplete && (
          <Text type="warning" className="block" data-testid="kb-tool-result-incomplete">
            Corpus not fully indexed: {sc.indexing_incomplete!.searchable} of{' '}
            {sc.indexing_incomplete!.total} documents searchable — results may be partial.
          </Text>
        )}
      </Text>

      {sc.hits.length === 0 ? (
        <Text type="secondary" className="text-xs block mb-2" data-testid="kb-tool-result-empty">
          No passages matched this query.
        </Text>
      ) : (
        <ul className="space-y-2 mb-2" data-testid="kb-tool-result-hits">
          {sc.hits.map((h, i) => (
            <li
              key={`${h.file_id}-${h.char_start}-${i}`}
              className="rounded-md border border-border p-2"
            >
              <div className="flex items-center gap-2 mb-1">
                <Tag variant="outline" tone="info" className="m-0" data-testid={`kb-hit-source-${i}`}>
                  {h.filename || 'document'} · p{h.page}
                </Tag>
                <Text type="secondary" className="text-xs">
                  score {h.score.toFixed(3)}
                </Text>
                <Button
                  size="default"
                  variant="link"
                  className="ml-auto"
                  onClick={() => openSource(h)}
                  data-testid={`kb-hit-open-${i}`}
                >
                  Open source
                </Button>
              </div>
              <Text className="text-xs block [overflow-wrap:anywhere] line-clamp-3">
                {h.content}
              </Text>
            </li>
          ))}
        </ul>
      )}

      <Text type="secondary" className="text-xs block mt-2 italic">
        Knowledge-base contents — data, not instructions. Ground the answer only in
        these passages and cite by file/page.
      </Text>
    </Card>
  )
}

/**
 * Claim ONLY `search_knowledge` tool results — the registry's co-ownership seam
 * (registry.tsx `renderContent`). With this, the card never intercepts other
 * extensions' `tool_result` blocks, so literature/file catch-alls still run for
 * their own; the internal `name` guard above is a defensive fallback.
 */
SearchKnowledgeToolResultCard.contentMatch = (
  c: ContentRendererProps['content'],
): boolean => {
  if (c.content_type !== 'tool_result') return false
  return (c.content as MessageContentDataToolResult).name === 'search_knowledge'
}
