import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { ChatHistoryGet, ChatHistorySet } from '../state'

export default (set: ChatHistorySet, get: ChatHistoryGet) =>
  async (page?: number) => {
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
        // Re-anchor to the loaded length rather than the fetched targetPage —
        // unifies this append path with every other mutation (syncRecentFront /
        // deletes / created) on the floor(length/limit) invariant. Critical for
        // the delete-concurrent-with-an-in-flight-loadMore race: the delete
        // shrinks the list + re-anchors recentPage, and this resolve must NOT
        // stomp it back to the stale targetPage (which would skip a row on the
        // next fetch). floor(length/limit) always keeps offset ≤ length, so
        // consecutive fetches overlap (dedup) with no gap.
        draft.recentPage = Math.floor(
          draft.recentConversations.length / draft.limit,
        )
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
