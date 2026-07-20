
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (
      conversationId: string,
      delayMs: number = 5 * 60 * 1000,
    ) => {
      get().cancelCacheClear(conversationId)

      const timer = setTimeout(() => {
        get().clearConversationCache(conversationId)
        console.log(
          `[Chat.store] Auto-cleared cache for conversation: ${conversationId}`,
        )
      }, delayMs)

      set(state => {
        const newTimers = new Map(state.cacheClearTimers)
        newTimers.set(conversationId, timer)
        return { cacheClearTimers: newTimers }
      })
      const delayMinutes = Math.round(delayMs / 60000)
      console.log(
        `[Chat.store] Scheduled cache clear for ${conversationId} in ${delayMinutes} minute(s)`,
      )
    }
}
