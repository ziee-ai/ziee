import {
  createExtension,
  type ChatExtension,
} from '@/modules/chat/core/extensions'

/**
 * Title Extension
 * Handles automatic conversation title generation via SSE events
 */
const titleExtension: ChatExtension = createExtension({
  name: 'title',
  description: 'Handles automatic conversation title generation',
  priority: 60,

  // No store needed - stateless extension

  // Type-safe SSE event handlers
  sseEventHandlers: {
    titleUpdated: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamTitleUpdatedData
      // get() returns Chat store state, set() updates Chat store state
      console.log('[Title Extension] Title updated:', data.title)

      const currentConversation = get().conversation
      if (!currentConversation) {
        console.warn('[Title Extension] No active conversation')
        return
      }

      // Update conversation title in Chat store (object replacement pattern without Immer)
      set((state: any) => ({
        conversation: state.conversation
          ? { ...state.conversation, title: data.title }
          : state.conversation,
      }))

      // Emit event for ChatHistory to update
      const { Stores } = await import('@/core/stores')
      await Stores.EventBus.emit({
        type: 'conversation.titleUpdated',
        data: {
          conversationId: currentConversation.id,
          title: data.title,
        },
      })

      console.log('[Title Extension] Updated title and emitted event:', data.title)
    },
  },
})

export default titleExtension
