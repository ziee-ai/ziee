import { Stores } from '@ziee/framework/stores'
import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { TaskListChecklist } from '@/modules/chat/components/agent-activity/TaskListChecklist'

/**
 * Renders the agent's live task-list checklist (ITEM-36) inline in the assistant
 * turn it belongs to. Registered in the `message_footer` slot, so it mounts once
 * per message inside `MessageContext.Provider`; it reads the message from context
 * and looks up that message's latest task-list snapshot in `TaskListStore`.
 *
 * Pane-correct: binds to THIS pane's own `TaskListStore` (the VoiceStore pattern)
 * so a split pane shows ITS own stream's task list, not the focused pane's. The
 * committed `TaskListChecklist` returns null on an empty list, so a message with
 * no task list renders nothing.
 */
export function TaskListMessageFooter() {
  const msg = useMessageContext()
  const pane = useChatPaneOrNull()
  const store = ((pane?.store ?? Stores.Chat) as typeof Stores.Chat).TaskListStore
  // Reactive read (installs the useStore subscription) — must run every render,
  // BEFORE any early return, so the hook order stays stable.
  const { byMessage } = store

  if (!msg || msg.role !== 'assistant') return null

  return <TaskListChecklist items={byMessage[msg.id] ?? []} />
}
