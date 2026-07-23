
import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { SubAgentActivityCard } from '@/modules/chat/components/agent-activity/SubAgentActivityCard'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/** Empty snapshot for a message with no sub-agent activity — the committed
 *  `SubAgentActivityCard` returns null on zero children, so this renders nothing. */
const EMPTY_ACTIVITY = { children: [] } as const

/**
 * Renders the live delegated sub-agent activity card (ITEM-4) inline in the
 * assistant turn it belongs to. Registered in the `message_footer` slot, so it
 * mounts once per message inside `MessageContext.Provider`; it reads the message
 * from context and looks up that message's latest activity snapshot in
 * `SubAgentActivityStore`.
 *
 * Pane-correct: binds to THIS pane's own `SubAgentActivityStore` (the TaskList /
 * VoiceStore pattern) so a split pane shows ITS own stream's activity, not the
 * focused pane's. The committed `SubAgentActivityCard` returns null on an empty
 * child list, so a message with no fan-out renders nothing.
 */
export function SubAgentActivityMessageFooter() {
  const msg = useMessageContext()
  const pane = useChatPaneOrNull()
  const store = ((pane?.store ?? Chat) as typeof Chat)
    .SubAgentActivityStore
  // Reactive read (installs the useStore subscription) — must run every render,
  // BEFORE any early return, so the hook order stays stable.
  const { byMessage } = store

  if (!msg || msg.role !== 'assistant') return null

  return <SubAgentActivityCard activity={byMessage[msg.id] ?? EMPTY_ACTIVITY} />
}
