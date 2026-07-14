import { BookOpen } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import { SkillConversationDrawer } from '@/modules/skill/components/SkillConversationDrawer'

/**
 * "+" dropdown entry for the per-conversation skills opt-out (Path B).
 * Opening it flips `Stores.SkillConversationDrawer.open`; the Dialog itself is
 * hosted by `SkillConversationDrawerHost` from an always-mounted composer slot
 * — NOT here. The "+" dropdown unmounts its items when it closes (which this
 * onClick triggers), so a Dialog rendered inside this item would be torn down
 * before it could appear. Hidden until a conversation exists.
 */
export function SkillMenuItem() {
  const { close } = usePlusDropdown()
  const conversation = Stores.Chat.conversation

  if (!conversation?.id) return null

  return (
    <PlusMenuItem
      data-testid="skill-conversation-menu-item"
      aria-label="Skills in this chat"
      icon={<BookOpen />}
      label="Skills in this chat"
      onClick={() => {
        Stores.SkillConversationDrawer.openDrawer()
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
  const conversation = Stores.Chat.conversation
  if (!conversation?.id) return null
  return <SkillConversationDrawer conversationId={conversation.id} />
}
