import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import {
  effectiveAssistantId,
  newChatAssistantKey,
} from '@/modules/assistant/stores/AssistantPicker.store'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

/**
 * AssistantStatusChip Component
 * Shows the selected assistant as a purple tag in the status row
 */
export function AssistantStatusChip() {
  // Permission gate (layer 4) — see AssistantMenuItem.
  const canRead = usePermission(Permissions.AssistantsRead)
  // Per-conversation selection (ITEM-5): `selectedAssistantId` is derived below
  // from `selectedByConversation[key]`, not read globally off the store.
  const { selectedByConversation, availableAssistants } = Stores.AssistantPicker
  // Key by THIS pane's conversation (bridge-resolved). (ITEM-5)
  const pane = useChatPaneOrNull()
  const key =
    Stores.Chat.conversation?.id ?? newChatAssistantKey(pane?.paneId)
  // Effective id: an untouched new chat shows the user's default assistant chip.
  const selectedAssistantId = effectiveAssistantId(
    selectedByConversation,
    availableAssistants,
    key,
  )

  if (!canRead) return null
  if (!selectedAssistantId) return null

  const assistant = availableAssistants.find(
    (a: any) => a.id === selectedAssistantId,
  )
  if (!assistant) return null

  // DEPLOY-ONLY: render nothing. Everything ABOVE this line is deliberately
  // UNCHANGED — the component must still mount and touch `Stores.AssistantPicker`,
  // because that access is what lazily initializes the picker store and loads the
  // assistant catalog that `composeRequestFields` reads (synchronously) at send
  // time. Suppressing only the paint — rather than unregistering the slot — is
  // what keeps the assistant functionally active while hiding the chip.
  // See the matching note in ../extension.tsx.
  return null
}
