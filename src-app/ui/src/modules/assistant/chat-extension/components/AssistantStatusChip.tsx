import { Tag } from '@ziee/kit'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Bot } from 'lucide-react'
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
  // NOTE (deploy): this `Stores.AssistantPicker` access must stay ABOVE every
  // early return. The picker store initializes LAZILY on first access, and this
  // component's mount is what performs it — that access is what loads the
  // assistant catalog `composeRequestFields` reads (synchronously) at send time.
  // Returning null before it would leave the catalog empty for non-admins and
  // silently drop `assistant_id` from their requests.
  const { selectedByConversation, availableAssistants, clearAssistant } =
    Stores.AssistantPicker
  // DEPLOY-ONLY: the chip is ADMIN-ONLY — same scoping as the "Assistants"
  // settings entry (see SettingsPage.tsx). Normal users still get the assistant
  // applied to every chat; they just don't see or manage it.
  const { user } = Stores.Auth
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
  // DEPLOY-ONLY: hidden for non-admins (store access above already happened).
  if (!user?.is_admin) return null
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
