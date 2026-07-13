import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { type ConversationResponse, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { createStoreProxy } from '@/core/stores'

// This store mutates `selectedIds` (a Set) through immer, so the MapSet plugin
// must be enabled. Own it here rather than relying on another store's import
// happening to run first. `enableMapSet` is idempotent.
enableMapSet()

/** Conversation list sort order (mirrors the backend `sort` query param). */
export type ConversationSort = 'recent' | 'oldest' | 'alpha' | 'most_messages'

/**
 * ChatHistory Store — conversation list, content search, sort, pagination,
 * bulk operations.
 *
 * Search + sort resolve SERVER-SIDE: `searchQuery` matches a conversation's
 * title OR any message's text content (content isn't loaded client-side, so the
 * old client title-only filter couldn't do content search), and `sort` maps to
 * the backend `sort` param. Both flow through the single `loadConversations`
 * query, so pagination works with them.
 */
export const ChatHistory = defineStore('ChatHistory', {
  immer: true,
  state: {
    conversations: [] as ConversationResponse[],
    recentConversations: [] as ConversationResponse[],
    // Pagination (the /chats history list — search/sort mutable).
    page: 1,
    limit: 20,
    total: 0,
    hasMore: false,
    // Sidebar "recent chats" paging — a DEDICATED, always-unfiltered/recent-sort
    // paging cursor, decoupled from the search/sort-mutable history list above so
    // a /chats reload can never reset the accumulated (infinite-scrolled) sidebar.
    recentPage: 1,
    recentTotal: 0,
    recentHasMore: false,
    recentLoading: false,
    recentLoadingMore: false,
    recentInitialized: false,
    recentError: null as string | null,
    // Monotonic epoch bumped when the recent list is structurally reset (e.g. a
    // delete drains it to empty). An in-flight load captures the epoch and
    // discards its result if it changed underneath — so a stale page-N response
    // can't append onto a list that was reset mid-flight.
    recentLoadSeq: 0,
    // Search + sort state (both applied server-side).
    searchQuery: '',
    sort: 'recent' as ConversationSort,
    // Selection
    selectedIds: new Set<string>(),
    // Loading states
    loading: false,
    loadingMore: false,
    deleting: false,
    // A page-1 refresh (new search/sort) requested while a load was in flight —
    // re-run once the current load settles so the latest query isn't dropped.
    reloadQueued: false,
    error: null as string | null,
    isInitialized: false,
  },
  actions: (set, get) => {
    const loadConversations = async (page?: number) => {
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
        await loadConversations(1)
      }
    }

    // Sidebar "recent chats" loader — ALWAYS unfiltered + default (recent) sort,
    // its own cursor. Page 1 replaces; later pages append (dedup by id). Mirrors
    // `loadConversations`'s in-flight guard + shape, but writes only the `recent*`
    // fields so it never collides with the /chats history query.
    const loadRecentConversations = async (page?: number) => {
      // Same permission gate as the history fetch — the sidebar widget accesses
      // this store on every render.
      if (!hasPermissionNow(Permissions.ConversationsRead)) return
      const state = get()
      const targetPage = page ?? state.recentPage
      if (state.recentLoading || state.recentLoadingMore) return
      // Capture the epoch so a reset (delete-drains-to-empty) that happens while
      // this fetch is in flight causes us to discard the now-stale result.
      const seq = state.recentLoadSeq
      set({
        recentLoading: targetPage === 1,
        recentLoadingMore: targetPage > 1,
        recentError: null,
      })
      try {
        const response = await ApiClient.Conversation.list({
          page: targetPage,
          limit: state.limit,
          // No `search`, no `sort`: the server defaults to `recent`, so page 1 is
          // byte-identical to the pre-feature unfiltered request.
        })
        // Superseded by a mid-flight reset → drop this result entirely (don't
        // append a stale page onto a list that was drained + is being refilled).
        if (get().recentLoadSeq !== seq) return
        const pageItems = response.conversations
        set(draft => {
          let added: number
          if (targetPage === 1) {
            draft.recentConversations = pageItems
            added = pageItems.length
          } else {
            const seen = new Set(draft.recentConversations.map(c => c.id))
            const fresh = pageItems.filter(c => !seen.has(c.id))
            draft.recentConversations = [...draft.recentConversations, ...fresh]
            added = fresh.length
          }
          draft.recentPage = targetPage
          draft.recentTotal = response.total
          // End-detection is anchored on the SERVER page size, not just the
          // length<total heuristic — otherwise a drifted `recentTotal` (a
          // cross-device delete of an unloaded row, or a boundary row dropped by
          // dedup) would keep `recentHasMore` true forever and the widget's
          // last-item effect would hammer the API with empty tail pages. A short
          // page, or a later page that adds nothing new, means we've hit the end.
          const serverEnd = pageItems.length < state.limit
          const noProgress = targetPage > 1 && added === 0
          draft.recentHasMore =
            !serverEnd &&
            !noProgress &&
            draft.recentConversations.length < response.total
          draft.recentLoading = false
          draft.recentLoadingMore = false
          draft.recentInitialized = true
        })
      } catch (error) {
        // A superseded (reset mid-flight) failure is not this view's error.
        if (get().recentLoadSeq !== seq) return
        console.error('[ChatHistory] Failed to load recent conversations:', error)
        // Surface a retryable error instead of wedging on the spinner. The widget
        // shows an error+retry when the list is empty; a failed load-MORE (list
        // already populated) just clears the flag and keeps the loaded rows.
        set({
          recentLoading: false,
          recentLoadingMore: false,
          recentError: 'Failed to load conversations',
        })
      }
    }

    // Cross-device create: notify-only sync gives just {action,id}, so refetch
    // page 1 and MERGE the genuinely-new rows to the FRONT — never replace, so an
    // already-infinite-scrolled sidebar keeps its loaded older pages (a page-1
    // replace would collapse the accumulated list + jump the scroll).
    const syncRecentFront = async () => {
      if (!hasPermissionNow(Permissions.ConversationsRead)) return
      // Nothing accumulated yet ⇒ a plain first-page load is correct.
      if (!get().recentInitialized) {
        await loadRecentConversations(1)
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

    // After a delete drains the loaded sidebar list to empty while the server
    // still has rows, reload page 1 — the empty render has no virtual rows, so
    // the auto-load effect can't self-heal.
    const refillRecentIfEmptied = async () => {
      const s = get()
      if (s.recentConversations.length === 0 && s.recentHasMore) {
        // Bump the epoch (invalidating any in-flight loadMore so its stale page-N
        // result is discarded) and clear the in-flight flags so the page-1 reload
        // below isn't blocked by the load guard.
        set(draft => {
          draft.recentLoadSeq = draft.recentLoadSeq + 1
          draft.recentLoading = false
          draft.recentLoadingMore = false
        })
        await loadRecentConversations(1)
      }
    }

    return {
      loadConversations,
      loadRecentConversations,
      syncRecentFront,
      refillRecentIfEmptied,
      loadMoreRecent: async () => {
        const state = get()
        if (!state.recentHasMore || state.recentLoadingMore || state.recentLoading)
          return
        await loadRecentConversations(state.recentPage + 1)
      },
      // Clear a lingering load-MORE error (the widget calls this when the user
      // scrolls away from the failed bottom, so returning retries once instead
      // of a tight loop while pinned at the end).
      clearRecentError: () => set({ recentError: null }),
      loadNextPage: async () => {
        const state = get()
        if (!state.hasMore || state.loadingMore) return
        await loadConversations(state.page + 1)
      },
      setSearchQuery: (query: string) => {
        // Route search to the backend (title + message content). Reset to
        // page 1 for the new result set.
        set(draft => {
          draft.searchQuery = query
        })
        void loadConversations(1)
      },
      setSort: (sort: ConversationSort) => {
        set(draft => {
          draft.sort = sort
        })
        void loadConversations(1)
      },
      deleteConversation: async (id: string) => {
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
          await refillRecentIfEmptied()
          // Broadcast deletion so other widgets drop the row (closes audit F5).
          // Import-late to avoid a cycle through `@/core/stores`.
          const { Stores } = await import('@/core/stores')
          await Stores.EventBus.emit({ type: 'conversation.deleted', data: { conversationId: id } })
        } catch (error) {
          console.error('[ChatHistory] Failed to delete conversation:', error)
          set({ error: 'Failed to delete conversation', deleting: false })
          throw error
        }
      },
      bulkDelete: async () => {
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
          await refillRecentIfEmptied()
        } catch (error) {
          console.error('[ChatHistory] Failed to bulk delete conversations:', error)
          set({ error: 'Failed to delete selected conversations', deleting: false })
          throw error
        }
      },
      toggleSelection: (id: string) => {
        set(draft => {
          if (draft.selectedIds.has(id)) draft.selectedIds.delete(id)
          else draft.selectedIds.add(id)
        })
      },
      selectAll: () => {
        set(draft => {
          // The visible list IS the (server-filtered) `conversations` now.
          draft.conversations.forEach(conv => {
            draft.selectedIds.add(conv.id)
          })
        })
      },
      deselectAll: () => {
        set(draft => {
          draft.selectedIds.clear()
        })
      },
      updateConversationTitle: async (id: string, title: string) => {
        try {
          await ApiClient.Conversation.update({ id, title })
          set(draft => {
            const updateTitle = (conv: ConversationResponse) => {
              if (conv.id === id) conv.title = title
            }
            draft.conversations.forEach(updateTitle)
            draft.recentConversations.forEach(updateTitle)
          })
        } catch (error) {
          console.error('[ChatHistory] Failed to update conversation title:', error)
          set({ error: 'Failed to update conversation title' })
          throw error
        }
      },
      refreshConversations: async () => {
        await loadConversations(1)
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('conversation.created', event => {
      const { conversation } = event.data
      set(draft => {
        // Convert Conversation to ConversationResponse by adding message_count.
        const conversationResponse: ConversationResponse = { ...conversation, message_count: 0 }
        // recentConversations must always reflect the true most-recent list, so
        // prepend there regardless of the current view. Do NOT truncate: the
        // sidebar is now infinite-scroll paged, so already-loaded older pages
        // must survive a new-chat prepend (the old `.slice(0,20)` would drop
        // them). Bump `recentTotal` only when this is genuinely a new id.
        const alreadyInRecent = draft.recentConversations.some(
          c => c.id === conversation.id,
        )
        draft.recentConversations = [
          conversationResponse,
          ...draft.recentConversations.filter(c => c.id !== conversation.id),
        ]
        if (!alreadyInRecent) draft.recentTotal = draft.recentTotal + 1
        // The main `conversations` list may be a FILTERED (search) or non-recent
        // SORTED view. A brand-new empty conversation won't match a content
        // search and has no defined position under a non-recent sort, so only
        // optimistically insert it (and bump the total) in the unfiltered,
        // default-sort view; otherwise leave the result set to the next load.
        if (!draft.searchQuery.trim() && draft.sort === 'recent') {
          draft.conversations.unshift(conversationResponse)
          draft.total = draft.total + 1
        }
      })
    })
    on('conversation.titleUpdated', event => {
      const { conversationId, title } = event.data
      set(draft => {
        const updateTitle = (conv: ConversationResponse) => {
          if (conv.id === conversationId) conv.title = title
        }
        draft.conversations.forEach(updateTitle)
        draft.recentConversations.forEach(updateTitle)
      })
    })
    // Cross-device sync: notify-and-refetch — the event carries only
    // {action, id}, so reload the first page or drop it on delete.
    on('sync:conversation', async event => {
      const { action, id } = event.data
      if (action === 'delete') {
        set(draft => {
          // Only adjust `total` if the deleted conversation was actually in the
          // current (possibly search-FILTERED) result set. A cross-device delete
          // of a conversation that doesn't match the active search must not
          // decrement the filtered total (which would desync "Showing X of N"
          // and hasMore/Load-More).
          const wasPresent = draft.conversations.some(c => c.id === id)
          const wasInRecent = draft.recentConversations.some(c => c.id === id)
          draft.conversations = draft.conversations.filter(c => c.id !== id)
          draft.recentConversations = draft.recentConversations.filter(c => c.id !== id)
          // Prune the selection too, so a still-selected row can't be
          // double-counted by a later bulkDelete after a cross-device delete.
          draft.selectedIds.delete(id)
          if (wasPresent) draft.total = Math.max(0, draft.total - 1)
          if (wasInRecent) {
            draft.recentTotal = Math.max(0, draft.recentTotal - 1)
            draft.recentPage = Math.floor(
              draft.recentConversations.length / draft.limit,
            )
          }
        })
        // Cross-device delete that emptied the loaded list → refill (see above).
        void actions.refillRecentIfEmptied()
      } else {
        // Refetch the history list (page 1) and MERGE-prepend the sidebar's new
        // rows (preserving its accumulated infinite-scroll pages).
        await actions.loadConversations(1)
        await actions.syncRecentFront()
      }
    })
    // On (re)connect, resync to cover anything missed offline. A full page-1
    // replace of the recent list is correct here (fresh view after a gap).
    on('sync:reconnect', () => {
      void actions.loadConversations(1)
      void actions.loadRecentConversations(1)
    })
  },
})

export const useChatHistoryStore = ChatHistory.store
export const ChatHistoryStoreProxy = createStoreProxy(useChatHistoryStore)
