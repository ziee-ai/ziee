import { defineExtensionStore } from '@/modules/chat/core/extensions'

/**
 * CompactionStore — live "context compacted" marker state (ITEM-61 / DEC-137).
 *
 * The `historyReplaced` SSE frame carries `{ conversation_id, summary_upto }` with
 * NO message id, so the handler pins the marker to the CURRENT last message in the
 * pane (the streaming assistant row when a turn is active, else the last loaded
 * message). The `message_footer` slot renders a "Context compacted" divider under
 * that message.
 *
 * One instance per pane (injected as `Stores.Chat.CompactionStore`), so a split
 * pane shows ITS own compaction marker. Read reactively in the `message_footer`
 * slot via the pane-correct `Stores.Chat` bridge (mirrors the SubAgentActivity /
 * TaskList store pattern).
 *
 * EPHEMERAL BY DESIGN — fed only by live (non-replay) SSE frames; the durable
 * source of truth for what was summarized is the `conversation_summaries` row, so
 * a reload drops the in-memory marker (the compaction itself is unaffected).
 */
export const createCompactionStore = defineExtensionStore({
  immer: false,
  state: {
    /** The message id the latest "context compacted" divider is pinned under. */
    markerMessageId: null as string | null,
  },
  actions: set => ({
    /** Pin (last-wins) the compaction marker under `messageId`. */
    markCompacted: (messageId: string) => set(() => ({ markerMessageId: messageId })),
  }),
})

/** Augment ChatExtensionStores with CompactionStore
 *  (typed `Stores.Chat.CompactionStore`). */
declare module '../../types' {
  interface ChatExtensionStores {
    CompactionStore: ReturnType<typeof createCompactionStore>
  }
}
