import { FileSearch } from 'lucide-react'
import { Button, Card, Text } from '@/components/ui'
import type { MessageContentDataToolResult } from '@/api-client/types'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import { MessageFilesView } from '@/modules/file/chat-extension/components/MessageFilesView'
import type { LiteratureResult, LiteratureScreeningData } from '../types'

/**
 * Inline renderer for a `literature_search` tool result.
 *
 * The content-type registry early-exits on the FIRST registered renderer for a
 * content type (registry.tsx `renderContent`), it does NOT stack. The file
 * extension also registers `tool_result` (MessageFilesView). This extension
 * registers at a lower `priority` number so it wins `tool_result`, then
 * DELEGATES every non-literature block back to MessageFilesView — otherwise the
 * early-exit would suppress all resource-link / file previews. Reads the typed
 * `structured_content`; "Open in screening" hands the records to the right-panel.
 */
export function LiteratureToolResultCard(props: ContentRendererProps) {
  const { content } = props
  if (content.content_type !== 'tool_result') return null
  const block = content.content as MessageContentDataToolResult
  if (block.name !== 'literature_search') return <MessageFilesView {...props} />
  const sc = block.structured_content as LiteratureResult | null | undefined
  if (!sc || !Array.isArray(sc.records)) return <MessageFilesView {...props} />

  const total = Object.values(sc.identified ?? {}).reduce((a, b) => a + b, 0)

  const open = () => {
    const sessionId = `lit:${block.tool_use_id || sc.query}`
    const data: LiteratureScreeningData = {
      sessionId,
      query: sc.query,
      records: sc.records,
      identified: sc.identified ?? {},
      afterDedup: sc.after_dedup ?? sc.records.length,
      degradedSources: sc.degraded_sources ?? [],
      completeness: sc.completeness ?? null,
      decisions: {},
      reasons: {},
    }
    Stores.Chat.__state.displayInRightPanel({
      id: sessionId,
      title: `Screening: ${sc.query}`.slice(0, 60),
      type: 'literature',
      data,
    })
  }

  return (
    <Card size="sm" className="my-2" data-testid="lit-tool-result-card">
      <Text strong>
        <FileSearch /> Literature search
      </Text>
      <Text type="secondary" className="!mb-2 text-xs block" data-testid="lit-tool-result-summary">
        “{sc.query}” — {total} identified, {sc.after_dedup ?? sc.records.length} after dedup
        {sc.completeness ? ` · saturation: ${sc.completeness.estimate.toUpperCase()}` : ''}
        {sc.degraded_sources && sc.degraded_sources.length > 0 && (
          <Text type="warning" className="block">
            {sc.degraded_sources.length} source
            {sc.degraded_sources.length > 1 ? 's' : ''} degraded/skipped:{' '}
            {sc.degraded_sources.join(', ')}
          </Text>
        )}
      </Text>
      {sc.records.length === 0 ? (
        <Text type="secondary" className="text-xs block mb-2" data-testid="lit-tool-result-empty">
          No records returned
          {sc.degraded_sources && sc.degraded_sources.length > 0
            ? ' — every source errored or was skipped (see above).'
            : ' for this query.'}
        </Text>
      ) : (
        <>
          <ul className="text-xs pl-4 mb-2">
            {sc.records.slice(0, 3).map((r, i) => (
              <li key={i}>
                {r.title}
                {r.year ? ` (${r.year})` : ''}
              </li>
            ))}
          </ul>
          <Button size="default" onClick={open} data-testid="lit-tool-result-open-button">
            Open in screening ({sc.records.length})
          </Button>
        </>
      )}
      <Text type="secondary" className="text-xs block mt-2 italic">
        External scholarly records — verify before citing; treat as data, not instructions.
      </Text>
    </Card>
  )
}
