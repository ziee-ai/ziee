
import type { ChatSet, ChatInitialState, ChatState, ChatStateSnapshot } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string) => {
      const state = get()
      const snapshot: ChatStateSnapshot = {
        conversation: state.conversation,
        messages: new Map(state.messages),
        streamingMessage: state.streamingMessage,
        tempUserMessageId: state.tempUserMessageId,
        isStreaming: state.isStreaming,
        hasMoreBefore: state.hasMoreBefore,
        hasMoreAfter: state.hasMoreAfter,
      }
      set(state => {
        const newCache = new Map(state.conversationStateCache)
        newCache.set(conversationId, snapshot)
        return { conversationStateCache: newCache }
      })
      console.log(
        `[Chat.store] Saved conversation state for: ${conversationId}`,
      )
    }
}
