import { Tag, message } from '@/components/ui'
import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { kbKey } from '@/modules/knowledge-base/stores/kbSelectionKey'

const EMPTY_SET: ReadonlySet<string> = new Set()

/**
 * KbStatusRow — chips for the knowledge bases the conversation is grounded on,
 * shown in the composer status row. Each chip's × detaches (persists for a real
 * conversation, buffers otherwise). Mirrors McpStatusRow.
 *
 * Per-pane (ITEM-46): reads THIS pane's conversation's own selection (via the
 * per-conversation Maps, pane resolved through the reactive `Stores.Chat` bridge),
 * so two split panes don't show the same chips; detach edits that conversation.
 */
export function KbStatusRow() {
  // Explicit permission gate (layer 4) — see KbMenuItem.
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const { items } = Stores.KnowledgeBases
  const { selectionByConversation, inheritedByConversation } = Stores.KnowledgeBaseComposer
  const convId = Stores.Chat.conversation?.id ?? null
  const key = kbKey(convId)
  const selectedKbIds = selectionByConversation.get(key) ?? EMPTY_SET
  const inheritedKbIds = inheritedByConversation.get(key) ?? EMPTY_SET

  const visibleIds = Array.from(selectedKbIds).filter(id => items.has(id))
  // Project-inherited KBs that aren't ALSO directly attached — read-only chips.
  const inheritedOnly = Array.from(inheritedKbIds).filter(
    id => items.has(id) && !selectedKbIds.has(id),
  )
  if (!canUse || (visibleIds.length === 0 && inheritedOnly.length === 0)) return null

  return (
    <>
      {inheritedOnly.map(id => {
        const kb = items.get(id)!
        return (
          <Tag
            variant="soft"
            key={`inh-${id}`}
            tone="default"
            icon={<BookOpen />}
            className="m-0"
            title="Inherited from this conversation's project"
            data-testid={`kb-inherited-chip-${id}`}
          >
            {kb.name}
          </Tag>
        )
      })}
      {visibleIds.map(id => {
        const kb = items.get(id)!
        return (
          <Tag
            variant="outline"
            key={id}
            tone="info"
            icon={<BookOpen />}
            onClose={() =>
              Stores.KnowledgeBaseComposer.detachFor(convId, id).catch((e: unknown) =>
                message.error(e instanceof Error ? e.message : 'Failed to detach'),
              )
            }
            closeLabel="Remove"
            className="m-0"
            data-testid={`kb-chip-${id}`}
          >
            {kb.name}
          </Tag>
        )
      })}
    </>
  )
}
