import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { taskItemsFromFrame } from '@/modules/chat/components/agent-activity/agentActivity'
import { createTaskListStore } from './TaskList.store'
import { TaskListMessageFooter } from './components/TaskListMessageFooter'

/**
 * Task-list Extension (Group G — ITEM-36 live).
 *
 * Wires the backend's live `taskListChanged` chat SSE frame to the committed,
 * presentational `TaskListChecklist`. The frame carries `{ run_id, items }` with
 * NO message_id, so the handler keys the (idempotent, full-snapshot, last-wins)
 * item list to the IN-FLIGHT assistant message id — learned from the started/
 * content frames (`streamingMessage.id`, falling back to `streamingMessageId`) —
 * and stores it in this extension's own per-pane `TaskListStore`. The
 * `message_footer` slot then renders the checklist inline in that assistant turn.
 *
 * No FE feature flag is needed: when the backend agent path is off, no frame
 * arrives, `byMessage` stays empty, and nothing renders. The store injection +
 * SSE registration mirror the voice + mcp chat-extension patterns exactly.
 */
const taskListExtension: ChatExtension = createExtension({
  name: 'task-list',
  description: "Renders the agent's live task list inline in the assistant turn",
  priority: 55,

  store: {
    name: 'TaskListStore',
    createStore: createTaskListStore,
  },

  slots: {
    message_footer: { component: TaskListMessageFooter, order: 20 },
  },

  sseEventHandlers: {
    taskListChanged: (data, get) => {
      // Pane-scoped chat state (single-pane: primary; split: this pane's store).
      const state = get()
      // The in-flight assistant message id: prefer the rendered streaming row's
      // id (what the message_footer slot keys off), fall back to the id captured
      // from the started/content frames.
      const messageId =
        state.streamingMessage?.id ??
        (state as { streamingMessageId?: string | null }).streamingMessageId ??
        null
      if (!messageId) return
      // The injected per-pane store instance (same one the slot reads via the
      // Stores.Chat bridge). Present by streaming time; guard defensively.
      const store = (
        state as { TaskListStore?: ReturnType<typeof createTaskListStore> }
      ).TaskListStore
      store?.setForMessage(messageId, taskItemsFromFrame(data))
    },
  },
})

export default taskListExtension
