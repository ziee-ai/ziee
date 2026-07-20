import { ApiClient } from '@/api-client'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/chat'

export default (_set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async () => {
      // Generation runs server-side (detached); cancel it via the stop
      // endpoint. The detached task emits a `complete` (cancelled) frame which
      // `applyStreamFrame` then reconciles.
      const conversation = get().conversation
      const messageId = get().streamingMessageId
      if (conversation && messageId) {
        void ApiClient.Message.stopGeneration({
          conversation_id: conversation.id,
          assistant_message_id: messageId,
        })
      }
    }
}
