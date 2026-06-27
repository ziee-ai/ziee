import { ReadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { SkillConversationDrawer } from '@/modules/skill/components/SkillConversationDrawer'

/**
 * "+" dropdown entry for the per-conversation skills opt-out (Path B).
 * Mirrors McpMenuItem — opens a modal hosting ConversationSkillsPanel,
 * scoped to the active conversation. Hidden until a conversation exists
 * (the opt-out is per-conversation; nothing to scope to before then).
 */
export function SkillMenuItem() {
  const { close } = usePlusDropdown()
  const conversation = Stores.Chat.conversation

  if (!conversation?.id) return null
  const conversationId = conversation.id

  return (
    <>
      <div
        className="flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer text-foreground min-w-[180px]"
        onClick={() => {
          Stores.SkillConversationDrawer.openDrawer()
          close()
        }}
        onMouseEnter={e => {
          e.currentTarget.style.backgroundColor = 'bg-muted'
        }}
        onMouseLeave={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        <ReadOutlined className="text-base" />
        <span className="text-sm">Skills in this chat</span>
      </div>

      <SkillConversationDrawer conversationId={conversationId} />
    </>
  )
}
