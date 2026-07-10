import { Combobox, Tooltip } from '@/components/ui'
import { Stores } from '@/core/stores'
import { newChatAssistantKey } from '@/modules/assistant/stores/AssistantPicker.store'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

interface AssistantSelectorProps {
  disabled?: boolean
}

export function AssistantSelector({
  disabled = false,
}: AssistantSelectorProps) {
  // Access assistant store directly - reactive via store proxy
  const { availableAssistants, selectedByConversation, selectAssistant } =
    Stores.AssistantPicker
  // Key by THIS pane's conversation (bridge-resolved). (ITEM-5)
  const pane = useChatPaneOrNull()
  const key =
    Stores.Chat.conversation?.id ?? newChatAssistantKey(pane?.paneId)
  const selectedAssistantId = selectedByConversation[key]

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
