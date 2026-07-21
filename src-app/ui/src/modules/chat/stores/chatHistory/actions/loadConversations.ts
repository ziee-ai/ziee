import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { ChatHistoryGet, ChatHistorySet } from '../state'

export default (set: ChatHistorySet, get: ChatHistoryGet) =>
  async (page?: number) => {
    // Permission-gate the shell-eager-load fetch: the sidebar's recent-
    // conversations widget accesses this store on every render.
    if (!hasPermissionNow(Permissions.ConversationsRead)) return
    const state = get()
    const targetPage = page ?? state.page
    if (state.loading || state.loadingMore) {
      // A refresh requested mid-flight: remember to re-run when it settles so
      // the newest search/sort wins instead of being silently dropped.
      if (targetPage === 1) set({ reloadQueued: true })
      return
    }
    const search = state.searchQuery.trim()
    const sort = state.sort
    set({
      loading: targetPage === 1,
      loadingMore: targetPage > 1,
      error: null,
    })
    try {
      const response = await ApiClient.Conversation.list({
        page: targetPage,
        limit: state.limit,
        // Omit empty params so an unfiltered/default-sort request is
        // byte-identical to the pre-feature call.
        ...(search ? { search } : {}),
        ...(sort !== 'recent' ? { sort } : {}),
      })
      const pageItems = response.conversations
      set(draft => {
        if (targetPage === 1) {
          // First page - replace all. NOTE: this query owns ONLY the /chats
          // history list (`conversations`). The sidebar's `recentConversations`
          // is owned entirely by `loadRecentConversations`/`loadMoreRecent`
          // (its own always-unfiltered/recent cursor), so this path must NOT
          // touch it — otherwise a /chats reload would reset the accumulated,
          // infinite-scrolled sidebar back to one page.
          draft.conversations = pageItems
        } else {
          // Subsequent pages - append.
          draft.conversations = [...draft.conversations, ...pageItems]
        }
        draft.page = targetPage
        draft.hasMore = draft.conversations.length < response.total
        draft.total = response.total
        draft.loading = false
        draft.loadingMore = false
        draft.isInitialized = true
      })
    } catch (error) {
      console.error('[ChatHistory] Failed to load conversations:', error)
      set({ error: 'Failed to load conversations', loading: false, loadingMore: false })
    }
    // If a newer search/sort was requested while this load was in flight,
    // run it now (the guard above dropped it to avoid concurrent loads).
    if (get().reloadQueued) {
      set({ reloadQueued: false })
      // Re-import the factory to break the self-reference at runtime.
      const { default: lcFactory } = await import('./loadConversations')
      const lc = lcFactory(set, get)
      await lc(1)
    }
  }
