import { Tag } from '@/components/ui'
import { Bot } from 'lucide-react'
import { Stores } from '@/core/stores'
import { NEW_CHAT_ASSISTANT_KEY } from '@/modules/assistant/stores/AssistantPicker.store'

/**
 * AssistantStatusChip Component
 * Shows the selected assistant as a purple tag in the status row
 */
export function AssistantStatusChip() {
  const { selectedByConversation, availableAssistants, clearAssistant } =
    Stores.AssistantPicker
  // Key by THIS pane's conversation (bridge-resolved). (ITEM-5)
  const key = Stores.Chat.conversation?.id ?? NEW_CHAT_ASSISTANT_KEY
  const selectedAssistantId = selectedByConversation[key]

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
      onClose={() => clearAssistant(key)}
      closeLabel="Remove"
      className="m-0"
    >
      {assistant.name}
    </Tag>
  )
}
