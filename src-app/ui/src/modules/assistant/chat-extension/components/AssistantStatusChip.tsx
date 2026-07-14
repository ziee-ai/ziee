import { Tag } from '@ziee/kit'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Bot } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'

/**
 * AssistantStatusChip Component
 * Shows the selected assistant as a purple tag in the status row
 */
export function AssistantStatusChip() {
  // Permission gate (layer 4) — see AssistantMenuItem.
  const canRead = usePermission(Permissions.AssistantsRead)
  const { selectedAssistantId, availableAssistants, clearAssistant } =
    Stores.AssistantPicker

  if (!canRead) return null
  if (!selectedAssistantId) return null

  const assistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )
  if (!assistant) return null

  return (
    <Tag variant="outline"
      data-testid="assistant-status-chip"
      tone="info"
      icon={<Bot />}
      onClose={() => clearAssistant()}
      closeLabel="Remove"
      className="m-0"
    >
      {assistant.name}
    </Tag>
  )
}
