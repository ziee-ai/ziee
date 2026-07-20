
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (conversationId: string): Promise<boolean> => {
      const state = get()
      const snapshot = state.conversationStateCache.get(conversationId)
      if (!snapshot) {
        console.log(
          `[Chat.store] Cache miss for conversation: ${conversationId}`,
        )
        return false
      }

      set({
        conversation: snapshot.conversation,
        messages: new Map(snapshot.messages),
        streamingMessage: snapshot.streamingMessage,
        tempUserMessageId: snapshot.tempUserMessageId,
        isStreaming: snapshot.isStreaming,
        // Not snapshotted (transient live signals): a restored conversation is
        // not a fresh interruption / mid-finalize, so clear them to avoid a
        // stale suppression.
        lastTurnInterrupted: false,
        finalizingTurn: false,
        hasMoreBefore: snapshot.hasMoreBefore ?? false,
        hasMoreAfter: snapshot.hasMoreAfter ?? false,
        loadingOlder: false,
        loadingNewer: false,
      })
      console.log(
        `[Chat.store] Cache hit - restored conversation state for: ${conversationId}`,
      )
      return true
    }
}
