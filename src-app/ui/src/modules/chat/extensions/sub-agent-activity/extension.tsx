import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { subAgentActivityFromChildren } from '@/modules/chat/components/agent-activity/agentActivity'
import { createSubAgentActivityStore } from './SubAgentActivity.store'
import { SubAgentActivityMessageFooter } from './components/SubAgentActivityMessageFooter'

/**
 * Sub-Agent-Activity Extension (Group A — ITEM-4 / DEC-65 live).
 *
 * Wires the backend's live `subAgentActivity` chat SSE frame to the committed,
 * presentational `SubAgentActivityCard`. The frame carries `{ run_id, children }`
 * with NO message_id, so the handler keys the (idempotent, full-snapshot,
 * last-wins) child list to the IN-FLIGHT assistant message id — learned from the
 * started/content frames (`streamingMessage.id`, falling back to
 * `streamingMessageId`) — and stores it in this extension's own per-pane
 * `SubAgentActivityStore`. The `message_footer` slot then renders the card inline
 * in that assistant turn.
 *
 * No FE feature flag is needed: when the backend agent path never fans out, no
 * frame arrives, `byMessage` stays empty, and nothing renders (the card returns
 * null on zero children). The store injection + SSE registration mirror the
 * sibling task-list chat-extension pattern exactly.
 */
const subAgentActivityExtension: ChatExtension = createExtension({
  name: 'sub-agent-activity',
  description: "Renders the agent's delegated sub-agents inline in the assistant turn",
  priority: 56,

  store: {
    name: 'SubAgentActivityStore',
    createStore: createSubAgentActivityStore,
  },

  slots: {
    message_footer: { component: SubAgentActivityMessageFooter, order: 21 },
  },

  sseEventHandlers: {
    subAgentActivity: (data, get) => {
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
        state as {
          SubAgentActivityStore?: ReturnType<typeof createSubAgentActivityStore>
        }
      ).SubAgentActivityStore
      store?.setForMessage(messageId, subAgentActivityFromChildren(data.children))
    },
  },
})

export default subAgentActivityExtension
