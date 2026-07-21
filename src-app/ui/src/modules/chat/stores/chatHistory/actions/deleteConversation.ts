import { ApiClient } from '@/api-client'
import type { ChatHistoryGet, ChatHistorySet } from '../state'
import refillRecentFactory from './refillRecentIfEmptied'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const refillRecent = refillRecentFactory(set, get)
  return async (id: string) => {
    set({ deleting: true, error: null })
    try {
      await ApiClient.Conversation.delete({ id })
      set(draft => {
        // deleteConversation is invoked from several surfaces (recent-
        // conversations widget, project lists, …) where the target may not
        // be in this store's current (search-filtered) list — decrement the
        // total only when it actually was, or the filtered "Showing X of N"
        // and hasMore desync (same guard as the sync-delete path).
        const wasPresent = draft.conversations.some(conv => conv.id === id)
        const wasInRecent = draft.recentConversations.some(conv => conv.id === id)
        draft.conversations = draft.conversations.filter(conv => conv.id !== id)
        draft.recentConversations = draft.recentConversations.filter(conv => conv.id !== id)
        draft.selectedIds.delete(id)
        if (wasPresent) draft.total = Math.max(0, draft.total - 1)
        // Keep the sidebar's paging counter honest so `recentHasMore` doesn't
        // desync after a row is removed from the accumulated recent list, and
        // re-anchor the page cursor to the shrunk length so the next
        // loadMoreRecent re-fetches from a limit-aligned boundary and dedup
        // recovers the row that would otherwise be skipped past the offset.
        if (wasInRecent) {
          draft.recentTotal = Math.max(0, draft.recentTotal - 1)
          draft.recentPage = Math.floor(
            draft.recentConversations.length / draft.limit,
          )
        }
        draft.deleting = false
      })
      // If the delete emptied the loaded sidebar list while more exist
      // server-side, refill page 1 — the widget renders the empty state with
      // NO virtual rows, so the last-item auto-load effect can never fire.
      await refillRecent()
      // Broadcast deletion so other widgets drop the row (closes audit F5).
      // Import-late to avoid a cycle through `@ziee/framework/stores`.
      const { Stores } = await import('@ziee/framework/stores')
      await Stores.EventBus.emit({ type: 'conversation.deleted', data: { conversationId: id } })
    } catch (error) {
      console.error('[ChatHistory] Failed to delete conversation:', error)
      set({ error: 'Failed to delete conversation', deleting: false })
      throw error
    }
  }
}
