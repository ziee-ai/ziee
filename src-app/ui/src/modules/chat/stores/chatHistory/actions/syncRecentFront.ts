import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { ChatHistoryGet, ChatHistorySet } from '../state'
import loadRecentFactory from './loadRecentConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadRecent = loadRecentFactory(set, get)
  return async () => {
    if (!hasPermissionNow(Permissions.ConversationsRead)) return
    // Nothing accumulated yet ⇒ a plain first-page load is correct.
    if (!get().recentInitialized) {
      await loadRecent(1)
      return
    }
    try {
      const response = await ApiClient.Conversation.list({
        page: 1,
        limit: get().limit,
      })
      set(draft => {
        const seen = new Set(draft.recentConversations.map(c => c.id))
        const fresh = response.conversations.filter(c => !seen.has(c.id))
        draft.recentConversations = [...fresh, ...draft.recentConversations]
        draft.recentTotal = Math.max(
          response.total,
          draft.recentConversations.length,
        )
        // Re-anchor the page cursor to the grown length (same as the delete
        // paths). Without this, once accumulated front-prepends reach `limit`
        // the next loadMoreRecent(recentPage+1) fetches a server page fully
        // overlapping already-loaded rows → added===0 → the no-progress guard
        // would wrongly mark recentHasMore=false and strand the older pages.
        draft.recentPage = Math.floor(
          draft.recentConversations.length / draft.limit,
        )
        draft.recentHasMore =
          draft.recentConversations.length < draft.recentTotal
      })
    } catch (error) {
      console.error('[ChatHistory] Failed to sync recent front:', error)
    }
  }
}
