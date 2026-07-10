import { FileText } from 'lucide-react'
import { Button, Card, Text } from '@/components/ui'
import {
  type MessageContent,
  type MessageContentDataToolResult,
  type OpenDoc,
  Permissions,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import { OFFICE_DOCS_PANEL_ID } from '../stores/officeBridgeSync'

/** The office tool whose result this card renders. */
const LIST_OPEN_DOCUMENTS = 'list_open_documents'

/** Shape of a `list_open_documents` result's structuredContent (mirrors the
 *  Rust `{ documents: Vec<OpenDoc> }` the handler emits). */
interface ListOpenDocumentsResult {
  documents: OpenDoc[]
}

function toolResultBlock(content: MessageContent): MessageContentDataToolResult | null {
  if (content.content_type !== 'tool_result') return null
  return content.content as MessageContentDataToolResult
}

/**
 * Inline card for a `list_open_documents` tool result: a compact
 * "N open document(s)" summary with a button that opens the "Open Office
 * documents" right-panel, seeding it with the enumerated documents.
 *
 * Claims ONLY its own blocks via the registry's static `contentMatch` seam
 * (mirrors `WorkflowWorkspaceRunCard`), so every other `tool_result` falls
 * through to the file / literature renderers unchanged.
 */
export function OpenDocumentsToolResultCard(props: ContentRendererProps) {
  // Frontend-hidden gate (mirrors the store's data self-gate + the backend
  // `office_bridge::use` perm): a user without the perm never sees the office
  // UI — even for a seeded/leaked tool_result. Backstop to the `contentMatch`
  // gate below (which lets the block fall through to the default renderer).
  if (!hasPermissionNow(Permissions.OfficeBridgeUse)) return null
  const block = toolResultBlock(props.content)
  const sc = (block?.structured_content ?? null) as ListOpenDocumentsResult | null
  const documents = Array.isArray(sc?.documents) ? sc.documents : []

  const open = () => {
    Stores.Chat.displayInRightPanel<'office-bridge'>({
      id: OFFICE_DOCS_PANEL_ID,
      title: 'Open Office documents',
      type: 'office-bridge',
      data: { documents },
    })
    // Kick a fresh, permission-gated fetch so the panel reflects the current
    // state (documents may have opened/closed since the tool ran).
    void Stores.OfficeBridge.load()
  }

  return (
    <Card size="sm" className="my-2" data-testid="office-docs-tool-result-card">
      <Text strong>
        <FileText aria-hidden /> Open Office documents
      </Text>
      <Text
        type="secondary"
        className="!mb-2 block text-xs"
        data-testid="office-docs-tool-result-summary"
      >
        {documents.length} open document{documents.length === 1 ? '' : 's'}
      </Text>
      {documents.length > 0 && (
        <ul className="mb-2 ps-4 text-xs [overflow-wrap:anywhere]">
          {documents.slice(0, 3).map((d, i) => (
            <li key={i}>{d.name}</li>
          ))}
        </ul>
      )}
      <Button
        size="default"
        onClick={open}
        data-testid="office-docs-tool-result-open-button"
      >
        Open panel ({documents.length})
      </Button>
    </Card>
  )
}

/** Claim only `list_open_documents` tool results — and only for a user holding
 *  `office_bridge::use`, so a restricted user's block falls through to the
 *  default tool_result renderer (the office card is never claimed/shown). */
OpenDocumentsToolResultCard.contentMatch = (content: MessageContent): boolean => {
  if (!hasPermissionNow(Permissions.OfficeBridgeUse)) return false
  const block = toolResultBlock(content)
  return block?.name === LIST_OPEN_DOCUMENTS
}
