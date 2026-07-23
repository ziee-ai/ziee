import { ApiClient } from '@/api-client'
import type { ChatHistoryGet, ChatHistorySet } from '../state'
import refillRecentFactory from './refillRecentIfEmptied'
import { EventBus } from '@ziee/framework/stores'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const refillRecent = refillRecentFactory(set, get)
  return async () => {
    const state = get()
    if (state.selectedIds.size === 0) return
    // Capture before the `set` below clears the selection — the post-delete
    // broadcast still needs the ids.
    const deletedIds = Array.from(state.selectedIds)
    set({ deleting: true, error: null })
    try {
      await Promise.all(
        deletedIds.map(id => ApiClient.Conversation.delete({ id })),
      )
      set(draft => {
        const selectedIds = Array.from(draft.selectedIds)
        // Decrement by the number ACTUALLY removed from the list, not the
        // selection size — a selected row already removed by a concurrent
        // cross-device delete must not be counted twice.
        const before = draft.conversations.length
        draft.conversations = draft.conversations.filter(c => !selectedIds.includes(c.id))
        const removed = before - draft.conversations.length
        const beforeRecent = draft.recentConversations.length
        draft.recentConversations = draft.recentConversations.filter(
          c => !selectedIds.includes(c.id),
        )
        const removedRecent = beforeRecent - draft.recentConversations.length
        draft.total = Math.max(0, draft.total - removed)
        draft.recentTotal = Math.max(0, draft.recentTotal - removedRecent)
        if (removedRecent > 0) {
          draft.recentPage = Math.floor(
            draft.recentConversations.length / draft.limit,
          )
        }
        draft.selectedIds.clear()
        draft.deleting = false
      })
      await refillRecent()
      // Broadcast each deletion, same as the single-delete path — without this
      // a bulk delete left split panes holding a deleted conversation stale and
      // nothing could navigate an open `/chat/:id` away from a dead id (#168).
      for (const id of deletedIds) {
        await EventBus.emit({
          type: 'conversation.deleted',
          data: { conversationId: id },
        })
      }
    } catch (error) {
      console.error('[ChatHistory] Failed to bulk delete conversations:', error)
      set({ error: 'Failed to delete selected conversations', deleting: false })
      throw error
    }
  }
}
