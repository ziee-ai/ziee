import { BookOpen } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { SkillConversationDrawer } from '@/modules/skill/components/SkillConversationDrawer'
import { SkillConversationDrawer as SkillConversationDrawerStore } from '@/modules/skill/stores/skillConversationDrawer'

/**
 * "+" dropdown entry for the per-conversation skills opt-out (Path B).
 * Opening it sets `SkillConversationDrawerStore.openConversationId` to THIS
 * pane's conversation; the Dialog itself is
 * hosted by `SkillConversationDrawerHost` from an always-mounted composer slot
 * — NOT here. The "+" dropdown unmounts its items when it closes (which this
 * onClick triggers), so a Dialog rendered inside this item would be torn down
 * before it could appear. Hidden until a conversation exists.
 */
export function SkillMenuItem() {
  const { close } = usePlusDropdown()
  // Act on THIS pane's conversation, not the focused-pane bridge (split-safe,
  // mirrors McpMenuItem) — otherwise opening skills in pane B keyed the drawer to
  // pane A's conversation.
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Stores.Chat) as typeof Stores.Chat
  const conversation = chat.conversation

  if (!conversation?.id) return null
  const conversationId = conversation.id

  return (
    <PlusMenuItem
      data-testid="skill-conversation-menu-item"
      aria-label="Skills in this chat"
      icon={<BookOpen />}
      label="Skills in this chat"
      onClick={() => {
        SkillConversationDrawerStore.openDrawer(conversationId)
        close()
      }}
    />
  )
}

/**
 * Stable host for the per-conversation skills Dialog. Rendered from an
 * always-mounted composer slot so it survives the "+" dropdown closing.
 */
export function SkillConversationDrawerHost() {
  // Bind to THIS pane's conversation (split-safe) so each pane's drawer opens only
  // for its own conversation (the store is keyed by conversationId).
  const pane = useChatPaneOrNull()
  const chat = (pane?.store ?? Stores.Chat) as typeof Stores.Chat
  const conversation = chat.conversation
  if (!conversation?.id) return null
  return <SkillConversationDrawer conversationId={conversation.id} />
}
