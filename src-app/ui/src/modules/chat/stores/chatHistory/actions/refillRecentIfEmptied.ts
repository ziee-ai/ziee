import type { ChatHistoryGet, ChatHistorySet } from '../state'
import loadRecentFactory from './loadRecentConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadRecent = loadRecentFactory(set, get)
  return async () => {
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
      await loadRecent(1)
    }
  }
}
