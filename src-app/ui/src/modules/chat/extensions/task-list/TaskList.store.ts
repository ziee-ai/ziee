import { defineExtensionStore } from '@/modules/chat/core/extensions'
import type { TaskItemVM } from '@/modules/chat/components/agent-activity/agentActivity'

/**
 * TaskListStore — live agent task-list overlay state (Group G / ITEM-36).
 *
 * The `taskListChanged` SSE frame carries `{ run_id, items }` with NO message_id,
 * so the handler keys each snapshot to the IN-FLIGHT assistant message id (learned
 * from the started/content frames). Frames are idempotent FULL snapshots →
 * last-wins (a plain replace, never a merge), so this store just holds the latest
 * snapshot per assistant message.
 *
 * One instance per pane (injected as `Stores.Chat.TaskListStore`), so a split
 * pane's task list tracks ITS own stream. Read reactively in the `message_footer`
 * slot via the pane-correct `Stores.Chat` bridge (mirrors the VoiceStore pattern).
 */

/** Bound the retained snapshots so a long session can't grow the map without
 *  limit — task lists are ephemeral live overlays; only the newest turns matter.
 *  JS objects preserve insertion order, so the oldest keys evict first. */
const MAX_TRACKED_MESSAGES = 100

export const createTaskListStore = defineExtensionStore({
  immer: false,
  state: {
    /** assistantMessageId → its latest full task-list snapshot. */
    byMessage: {} as Record<string, TaskItemVM[]>,
  },
  actions: set => ({
    /** Replace (last-wins) the snapshot for one assistant message. */
    setForMessage: (messageId: string, items: TaskItemVM[]) =>
      set(state => {
        const next: Record<string, TaskItemVM[]> = {
          ...state.byMessage,
          [messageId]: items,
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

/** Augment ChatExtensionStores with TaskListStore (typed `Stores.Chat.TaskListStore`). */
declare module '../../types' {
  interface ChatExtensionStores {
    TaskListStore: ReturnType<typeof createTaskListStore>
  }
}
