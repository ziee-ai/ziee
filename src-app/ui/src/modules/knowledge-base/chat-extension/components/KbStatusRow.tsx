import { Tag, message } from '@ziee/kit'
import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { kbKey } from '@/modules/knowledge-base/stores/kbSelectionKey'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

const EMPTY_SET: ReadonlySet<string> = new Set()

/**
 * KbStatusRow — chips for the knowledge bases the conversation is grounded on,
 * shown in the composer status row. Each chip's × detaches (persists for a real
 * conversation, buffers otherwise). Mirrors McpStatusRow.
 *
 * Per-pane (ITEM-46/51): reads THIS pane's own conversation's selection via its own
 * store + paneId (the ConversationPage pattern), so two split panes don't show the
 * same chips — and, for a NEW chat, each pane reads its OWN per-pane pending buffer
 * (`kbKey(null, paneId)`), so a pending selection in one pane never appears in the
 * other. Detach edits that same key.
 */
export function KbStatusRow() {
  // Explicit permission gate (layer 4) — see KbMenuItem.
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const { items } = Stores.KnowledgeBases
  const { selectionByConversation, inheritedByConversation } = Stores.KnowledgeBaseComposer
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Stores.Chat) as typeof Stores.Chat
  const paneId = pane?.paneId ?? null
  const convId = chat.conversation?.id ?? null
  const key = kbKey(convId, paneId)
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
              Stores.KnowledgeBaseComposer.detachFor(convId, id, paneId).catch((e: unknown) =>
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
