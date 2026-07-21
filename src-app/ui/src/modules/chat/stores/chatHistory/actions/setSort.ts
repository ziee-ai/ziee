import type { ChatHistoryGet, ChatHistorySet } from '../state'
import type { ConversationSort } from '../state'
import loadConversationsFactory from './loadConversations'

export default (set: ChatHistorySet, get: ChatHistoryGet) => {
  const loadConversations = loadConversationsFactory(set, get)
  return (sort: ConversationSort) => {
    set(draft => {
      draft.sort = sort
    })
    void loadConversations(1)
  }
}
