import {
  createExtension,
  type ChatExtension,
  type SSEEvent,
  type HandleSSEEventResult,
  type SSEEventData,
} from '../../core/extensions'

/**
 * Title Extension
 * Handles automatic conversation title generation via SSE events
 */
const titleExtension: ChatExtension = createExtension({
  name: 'title',
  description: 'Handles automatic conversation title generation',
  priority: 60,

  // No store needed - stateless extension

  handleSSEEvent: async (event: SSEEvent): Promise<HandleSSEEventResult> => {
    // Handle title updated event
    if (event.event_type === 'titleUpdated') {
      const data = event.data as SSEEventData<'titleUpdated'>

      console.log('[Title Extension] Title updated:', data.title)

      // Update conversation title in store
      // TODO: Implement conversation store with updateConversationTitle method
      const updateTitle = () => {
        try {
          // Placeholder for conversation store integration
          // When conversation store is available, update it here:
          // Stores.Conversation.updateConversationTitle(conversationId, data.title)
          console.log(
            '[Title Extension] Title would be updated to:',
            data.title,
          )
        } catch (error) {
          console.error('[Title Extension] Failed to update title:', error)
        }
      }

      return {
        handled: true,
        uiUpdates: [updateTitle],
      }
    }

    return { handled: false }
  },
})

export default titleExtension
