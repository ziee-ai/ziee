import { Combobox, Tooltip } from '@ziee/kit'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import {
  effectiveAssistantId,
  newChatAssistantKey,
} from '@/modules/assistant/stores/AssistantPicker.store'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

interface AssistantSelectorProps {
  disabled?: boolean
}

export function AssistantSelector({
  disabled = false,
}: AssistantSelectorProps) {
  // Permission gate (layer 4) — see AssistantMenuItem.
  const canRead = usePermission(Permissions.AssistantsRead)
  // Access assistant store directly - reactive via store proxy
  const { availableAssistants, selectedByConversation, selectAssistant } =
    Stores.AssistantPicker
  // Key by THIS pane's conversation (bridge-resolved). (ITEM-5)
  const pane = useChatPaneOrNull()
  const key =
    Stores.Chat.conversation?.id ?? newChatAssistantKey(pane?.paneId)
  // Effective id: an untouched new chat shows the user's default assistant.
  const selectedAssistantId = effectiveAssistantId(
    selectedByConversation,
    availableAssistants,
    key,
  )

  if (!canRead) return null

  const handleChange = (assistantId: string) => {
    selectAssistant(key, assistantId)
  }

  // Build options for the combobox
  const options = availableAssistants.map((assistant: any) => ({
    label: assistant.name,
    value: assistant.id,
  }))

  // No assistants available: render a disabled, empty selector rather than
  // vanishing entirely, so the control stays present and self-explanatory.
  if (availableAssistants.length === 0) {
    return (
      <Tooltip content="No assistants available">
        <Combobox
          data-testid="assistant-selector"
          aria-label="Select Assistant"
          value={undefined}
          onChange={handleChange}
          options={[]}
          disabled
          placeholder="No assistants"
          className="min-w-[120px]"
          size="default"
          emptyText="No assistants available"
          searchPlaceholder="Search assistant"
        />
      </Tooltip>
    )
  }

  return (
    <Tooltip content="Select Assistant">
      <Combobox
        data-testid="assistant-selector"
        aria-label="Select Assistant"
        value={selectedAssistantId ?? undefined}
        onChange={handleChange}
        options={options}
        disabled={disabled}
        placeholder="Assistant"
        className="min-w-[120px]"
        size="default"
        emptyText="No assistants available"
        searchPlaceholder="Search assistant"
      />
    </Tooltip>
  )
}
