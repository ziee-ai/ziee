import type { ChatHistoryGet, ChatHistorySet } from '../state'
import loadConversationsFactory from './loadConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadConversations = loadConversationsFactory(set, get)
  return (query: string) => {
    // Route search to the backend (title + message content). Reset to
    // page 1 for the new result set.
    set(draft => {
      draft.searchQuery = query
    })
    void loadConversations(1)
  }
}
