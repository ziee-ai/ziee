import { Tag, message } from '@/components/ui'
import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * KbStatusRow — chips for the knowledge bases the conversation is grounded on,
 * shown in the composer status row. Each chip's × detaches (persists for a real
 * conversation, buffers otherwise). Mirrors McpStatusRow.
 */
export function KbStatusRow() {
  // Explicit permission gate (layer 4) — see KbMenuItem.
  const canUse = usePermission(Permissions.KnowledgeBaseUse)
  const { items } = Stores.KnowledgeBases
  const { selectedKbIds } = Stores.KnowledgeBaseComposer

  const visibleIds = Array.from(selectedKbIds).filter(id => items.has(id))
  if (!canUse || visibleIds.length === 0) return null

  return (
    <>
      {visibleIds.map(id => {
        const kb = items.get(id)!
        return (
          <Tag
            variant="outline"
            key={id}
            tone="info"
            icon={<BookOpen />}
            onClose={() =>
              Stores.KnowledgeBaseComposer.detach(id).catch((e: unknown) =>
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
