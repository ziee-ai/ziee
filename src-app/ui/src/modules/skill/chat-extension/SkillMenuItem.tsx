import { ReadOutlined } from '@ant-design/icons'
import { theme } from 'antd'
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
  const { token } = theme.useToken()
  const { close } = usePlusDropdown()
  const conversation = Stores.Chat.conversation

  if (!conversation?.id) return null
  const conversationId = conversation.id

  return (
    <>
      <div
        className="flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer focus-visible:outline focus-visible:outline-2"
        style={{ color: token.colorTextBase, minWidth: 180 }}
        role="button"
        tabIndex={0}
        aria-label="Skills in this chat"
        onClick={() => {
          Stores.SkillConversationDrawer.openDrawer()
          close()
        }}
        onKeyDown={e => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            Stores.SkillConversationDrawer.openDrawer()
            close()
          }
        }}
        onMouseEnter={e => {
          e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onMouseLeave={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
        onFocus={e => {
          e.currentTarget.style.backgroundColor = token.colorFillSecondary
        }}
        onBlur={e => {
          e.currentTarget.style.backgroundColor = 'transparent'
        }}
      >
        <ReadOutlined style={{ fontSize: 16 }} />
        <span style={{ fontSize: 14 }}>Skills in this chat</span>
      </div>

      <SkillConversationDrawer conversationId={conversationId} />
    </>
  )
}
