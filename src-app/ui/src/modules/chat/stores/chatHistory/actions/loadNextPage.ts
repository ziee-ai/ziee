import type { ChatHistoryGet, ChatHistorySet } from '../state'
import loadConversationsFactory from './loadConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadConversations = loadConversationsFactory(set, get)
  return async () => {
    const state = get()
    if (!state.hasMore || state.loadingMore) return
    await loadConversations(state.page + 1)
  }
}
