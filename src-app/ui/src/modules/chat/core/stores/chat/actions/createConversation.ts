import { ApiClient } from '@/api-client'
import type { ChatSet, ChatInitialState } from '@/modules/chat/core/stores/chat'

export default (set: ChatSet, _getRaw: () => ChatInitialState) => {
  return async (
      title?: string,
      modelId?: string,
      emitCreated: boolean = true,
    ) => {
      // Extensions can layer additional attribution onto the
      // freshly-created conversation via the
      // `afterCreateConversation` hook in sendMessage.
      set({ loading: true, error: null })

      try {
        const conversation = await ApiClient.Conversation.create({
          title: title,
          model_id: modelId,
        })
        set({ conversation, loading: false })

        if (emitCreated) {
          const { Stores } = await import('@ziee/framework/stores')
          await Stores.EventBus.emit({
            type: 'conversation.created',
            data: { conversation },
          })
        }

        return conversation
      } catch (error: any) {
        set({
          error: error.message || 'Failed to create conversation',
          loading: false,
        })
        throw error
      }
    }
}
