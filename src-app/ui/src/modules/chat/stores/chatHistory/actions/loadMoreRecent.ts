import type { ChatHistoryGet, ChatHistorySet } from '../state'
import loadRecentFactory from './loadRecentConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadRecent = loadRecentFactory(set, get)
  return async () => {
    const state = get()
    if (!state.recentHasMore || state.recentLoadingMore || state.recentLoading)
      return
    await loadRecent(state.recentPage + 1)
  }
}
