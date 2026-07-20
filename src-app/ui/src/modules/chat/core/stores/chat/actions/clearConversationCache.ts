
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string) => {
      get().cancelCacheClear(conversationId)
      set(state => {
        const newCache = new Map(state.conversationStateCache)
        newCache.delete(conversationId)
        return { conversationStateCache: newCache }
      })
      console.log(
        `[Chat.store] Cleared cache for conversation: ${conversationId}`,
      )
    }
}
