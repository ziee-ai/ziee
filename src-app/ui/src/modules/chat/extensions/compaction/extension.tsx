import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { createCompactionStore } from './Compaction.store'
import { CompactionMarker } from './components/CompactionMarker'
import { CompactButton } from './components/CompactButton'

/**
 * Compaction Extension (ITEM-61 / DEC-137).
 *
 * Two surfaces for context compaction:
 *   - a composer `toolbar_actions` button (`CompactButton`) that manually triggers
 *     `POST /conversations/{id}/compact`;
 *   - a `message_footer` marker (`CompactionMarker`) that renders a "context
 *     compacted" divider in the timeline when a `historyReplaced` SSE frame arrives
 *     (from the manual endpoint OR the agent loop's automatic compaction).
 *
 * The `historyReplaced` frame carries `{ conversation_id, summary_upto }` with NO
 * message id, so the handler pins the marker to the pane's CURRENT last message —
 * the streaming assistant row during a turn, else the last loaded message (a manual
 * `/compact` between turns). Ephemeral by design (SSE-only, no replay) — mirrors the
 * sibling sub-agent-activity / task-list extensions.
 */
const compactionExtension: ChatExtension = createExtension({
  name: 'compaction',
  description: 'Manual context-compaction affordance + a "context compacted" timeline marker',
  priority: 57,

  store: {
    name: 'CompactionStore',
    createStore: createCompactionStore,
  },

  slots: {
    // Sits just after the sub-agent-activity footer (order 21) in the message turn.
    message_footer: { component: CompactionMarker, order: 22 },
    // Sits just left of the schedule/loop button (order 84) in the composer toolbar.
    toolbar_actions: { component: CompactButton, order: 83 },
  },

  sseEventHandlers: {
    historyReplaced: (_data, get) => {
      const state = get()
      // Pin the marker to the pane's current last message: the streaming assistant
      // row when a turn is active, else the last loaded message (manual /compact
      // between turns). `messages` is an insertion-ordered Map.
      const loadedKeys = Array.from(
        (state.messages as Map<string, { id: string }>).keys(),
      )
      const lastLoaded = loadedKeys.length ? loadedKeys[loadedKeys.length - 1] : null
      const messageId =
        state.streamingMessage?.id ??
        (state as { streamingMessageId?: string | null }).streamingMessageId ??
        lastLoaded ??
        null
      if (!messageId) return
      const store = (
        state as { CompactionStore?: ReturnType<typeof createCompactionStore> }
      ).CompactionStore
      store?.markCompacted(messageId)
    },
  },
})

export default compactionExtension
