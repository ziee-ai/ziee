
import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Renders a "Context compacted" divider inline in the timeline (ITEM-61 / DEC-137),
 * under the message the compaction was pinned to. Registered in the `message_footer`
 * slot, so it mounts once per message inside `MessageContext.Provider`; it reads the
 * message from context and shows the divider only when that message is the pane's
 * current `markerMessageId`.
 *
 * Pane-correct: binds to THIS pane's own `CompactionStore` so a split pane shows ITS
 * own marker, not the focused pane's.
 */
export function CompactionMarker() {
  const msg = useMessageContext()
  const pane = useChatPaneOrNull()
  const store = ((pane?.store ?? Chat) as typeof Chat).CompactionStore
  // Reactive read (installs the subscription) — before any early return so the
  // hook order stays stable.
  const { markerMessageId } = store

  if (!msg || msg.id !== markerMessageId) return null

  return (
    <div
      data-testid="chat-history-replaced-marker"
      className="my-3 flex items-center gap-2 text-xs text-muted-foreground"
      role="separator"
      aria-label="Context compacted"
    >
      <div className="h-px flex-1 bg-border" />
      <span className="shrink-0">Context compacted</span>
      <div className="h-px flex-1 bg-border" />
    </div>
  )
}
