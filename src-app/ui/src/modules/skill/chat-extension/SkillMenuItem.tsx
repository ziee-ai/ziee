import { BookOpen } from 'lucide-react'
import { usePlusDropdown } from '@/modules/chat/components/PlusDropdownContext'
import { PlusMenuItem } from '@/modules/chat/components/PlusMenuItem'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { Suspense, lazy } from 'react'
import { SkillConversationDrawer as SkillConversationDrawerStore } from '@/modules/skill/stores/skillConversationDrawer'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'

// Lazy body: SkillConversationDrawer pulls in SkillDetailDrawer + the skills
// panel — all opened on demand from the composer, so they must not ride the
// chat bundle. The drawer store is already loaded here, so gating the mount on
// its open state is free; mount the body only once a skills drawer is opened.
const SkillConversationDrawer = lazy(() =>
  import('@/modules/skill/components/SkillConversationDrawer').then(m => ({
    default: m.SkillConversationDrawer,
  })),
)
import { Chat } from '@/modules/chat/core/stores/chatBridge'

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
  const chat = (pane?.store ?? Chat) as typeof Chat
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
  const chat = (pane?.store ?? Chat) as typeof Chat
  const conversation = chat.conversation
  // Gate the lazy body on THIS pane's drawer being open (kept mounted briefly
  // after close for the exit animation). Reads the already-loaded store, so the
  // heavy chunk downloads only when the user opens the skills drawer here.
  const open = useDelayedFalse(
    () => SkillConversationDrawerStore.openConversationId === conversation?.id,
  )
  if (!conversation?.id || !open) return null
  return (
    <Suspense fallback={null}>
      <SkillConversationDrawer conversationId={conversation.id} />
    </Suspense>
  )
}
