import type { ChatHistorySet } from '../state'
import loadConversationsFactory from './loadConversations'

export default (set: ChatHistorySet, get: () => { limit: number }) => {
  const loadConversations = loadConversationsFactory(set, get as never)
  return async () => {
    await loadConversations(1)
  }
}
