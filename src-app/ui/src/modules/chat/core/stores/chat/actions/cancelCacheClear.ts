
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string) => {
      const state = get()
      const timer = state.cacheClearTimers.get(conversationId)
      if (timer) {
        clearTimeout(timer)
        set(state => {
          const newTimers = new Map(state.cacheClearTimers)
          newTimers.delete(conversationId)
          return { cacheClearTimers: newTimers }
        })
        console.log(
          `[Chat.store] Cancelled cache clear for conversation: ${conversationId}`,
        )
      }
    }
}
