import { Tag } from '@/components/ui'
import { Bot } from 'lucide-react'
import { Stores } from '@/core/stores'

/**
 * AssistantStatusChip Component
 * Shows the selected assistant as a purple tag in the status row
 */
export function AssistantStatusChip() {
  const { selectedAssistantId, availableAssistants, selectAssistant } =
    Stores.AssistantPicker

  if (!selectedAssistantId) return null

  const assistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )
  if (!assistant) return null

  return (
    <Tag
      data-testid="assistant-status-chip"
      tone="info"
      icon={<Bot />}
      onClose={() => selectAssistant(null as any)}
      closeLabel="Remove"
      className="m-0"
    >
      {assistant.name}
    </Tag>
  )
}
