import { useMemo, useState } from 'react'
import { Card, Empty, Combobox, Spin, Paragraph } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { CoreMemoryBlocksEditor } from '@/modules/memory/components/CoreMemoryBlocksEditor'

const READ_PERM = Permissions.CoreMemoryRead

/**
 * Per-assistant core memory editor. Lets the user pick which assistant
 * to manage core-memory blocks for, then renders the editor for that
 * assistant. Hidden entirely if no `memory::core::read`.
 */
export function CoreMemorySection() {
  const canRead = usePermission(READ_PERM)
  const { assistants: assistantsMap, loading } = Stores.UserAssistants
  const [assistantId, setAssistantId] = useState<string | null>(null)

  const assistants = useMemo(
    () => Array.from(assistantsMap.values()),
    [assistantsMap],
  )

  if (!canRead) return null

  return (
    <Card title="Per-assistant core memory">
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Core-memory blocks (Letta-style) are prepended to a specific
        assistant&rsquo;s system prompt on every turn. Use them for
        persona, standing instructions, or context you want the
        assistant to always have. Each block is private to you.
      </Paragraph>

      <div className="mb-4">
        {loading ? (
          <Spin label="Loading assistants" />
        ) : assistants.length === 0 ? (
          <Empty
            description="No assistants yet"
          />
        ) : (
          <Combobox
            className="w-full"
            placeholder="Pick an assistant"
            value={assistantId ?? undefined}
            onChange={(v: string) => setAssistantId(v ?? null)}
            options={assistants.map((a) => ({
              value: a.id,
              label: a.name,
            }))}
            emptyText="No assistants found"
            searchPlaceholder="Search assistants"
          />
        )}
      </div>

      {assistantId && <CoreMemoryBlocksEditor assistantId={assistantId} />}
    </Card>
  )
}
