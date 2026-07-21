import { ApiClient } from '@/api-client'
import type { ChatHistoryGet, ChatHistorySet } from '../state'
import refillRecentFactory from './refillRecentIfEmptied'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const refillRecent = refillRecentFactory(set, get)
  return async () => {
    const state = get()
    if (state.selectedIds.size === 0) return
    set({ deleting: true, error: null })
    try {
      await Promise.all(
        Array.from(state.selectedIds).map(id => ApiClient.Conversation.delete({ id })),
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
    } catch (error) {
      console.error('[ChatHistory] Failed to bulk delete conversations:', error)
      set({ error: 'Failed to delete selected conversations', deleting: false })
      throw error
    }
  }
}
