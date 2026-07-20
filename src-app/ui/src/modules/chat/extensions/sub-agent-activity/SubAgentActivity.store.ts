import { defineExtensionStore } from '@/modules/chat/core/extensions'
import type { SubAgentActivityVM } from '@/modules/chat/components/agent-activity/agentActivity'

/**
 * SubAgentActivityStore — live delegated sub-agent activity overlay state
 * (Group A / ITEM-4 / DEC-65).
 *
 * The `subAgentActivity` SSE frame carries `{ run_id, children }` with NO
 * message_id, so the handler keys each snapshot to the IN-FLIGHT assistant
 * message id (learned from the started/content frames). Frames are idempotent
 * FULL snapshots → last-wins (a plain replace, never a merge), so this store
 * just holds the latest activity snapshot per assistant message.
 *
 * One instance per pane (injected as `Stores.Chat.SubAgentActivityStore`), so a
 * split pane's sub-agent card tracks ITS own stream. Read reactively in the
 * `message_footer` slot via the pane-correct `Stores.Chat` bridge (mirrors the
 * TaskListStore / VoiceStore pattern).
 */

/** Bound the retained snapshots so a long session can't grow the map without
 *  limit — sub-agent activity is an ephemeral live overlay; only the newest
 *  turns matter. JS objects preserve insertion order, so the oldest keys evict
 *  first. */
const MAX_TRACKED_MESSAGES = 100

export const createSubAgentActivityStore = defineExtensionStore({
  immer: false,
  state: {
    /** assistantMessageId → its latest full sub-agent activity snapshot. */
    byMessage: {} as Record<string, SubAgentActivityVM>,
  },
  actions: set => ({
    /** Replace (last-wins) the snapshot for one assistant message. */
    setForMessage: (messageId: string, activity: SubAgentActivityVM) =>
      set(state => {
        const next: Record<string, SubAgentActivityVM> = {
          ...state.byMessage,
          [messageId]: activity,
        }
        const keys = Object.keys(next)
        if (keys.length > MAX_TRACKED_MESSAGES) {
          for (const stale of keys.slice(0, keys.length - MAX_TRACKED_MESSAGES)) {
            delete next[stale]
          }
        }
        return { byMessage: next }
      }),
  }),
})

/** Augment ChatExtensionStores with SubAgentActivityStore
 *  (typed `Stores.Chat.SubAgentActivityStore`). */
declare module '../../types' {
  interface ChatExtensionStores {
    SubAgentActivityStore: ReturnType<typeof createSubAgentActivityStore>
  }
}
