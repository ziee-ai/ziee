import { ApiClient } from '@/api-client'
import type { ChatSet, ChatInitialState, ChatState } from '@/modules/chat/core/stores/Chat.store'

export default (set: ChatSet, getRaw: () => ChatInitialState) => {
  const get = getRaw as unknown as () => ChatState
  return async (updates: { title?: string }) => {
      const { conversation } = get()
      if (!conversation) {
        set({ error: 'No active conversation' })
        return
      }

      try {
        await ApiClient.Conversation.update({
          id: conversation.id,
          ...updates,
        })

        set(state => ({
          conversation: state.conversation
            ? { ...state.conversation, ...updates }
            : null,
        }))

        if (updates.title !== undefined) {
          const { Stores } = await import('@ziee/framework/stores')
          await Stores.EventBus.emit({
            type: 'conversation.titleUpdated',
            data: {
              conversationId: conversation.id,
              title: updates.title,
            },
          })
        }
      } catch (error: any) {
        set({
          error: error.message || 'Failed to update conversation',
        })
        throw error
      }
    }
}
