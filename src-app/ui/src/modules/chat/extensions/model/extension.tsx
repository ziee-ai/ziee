import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { createModelStore } from '@/modules/chat/extensions/model/Model.store'
import { ModelSelector } from '@/modules/chat/extensions/model/components/ModelSelector'
import type { Conversation } from '@/api-client/types'

/**
 * Model Extension
 * Handles model selection for chat messages
 */
const modelExtension: ChatExtension = createExtension({
  name: 'model',
  description: 'Handles model selection for chat messages',
  priority: 10, // High priority - before text (5)

  /**
   * Store for managing model selection
   */
  store: {
    name: 'ModelStore',
    createStore: createModelStore,
  },

  /**
   * Provide model_id to request
   */
  composeRequestFields: async () => {
    const { Stores } = await import('@/core/stores')

    const modelId = Stores.Chat.__state.ModelStore.getModelId()
    if (!modelId) {
      throw new Error('No model selected')
    }

    return {
      model_id: modelId,
    }
  },

  slots: {
    toolbar_model: { component: ModelSelector, order: 0 },
  },

  /**
   * Sync model selection when conversation loads or switches
   */
  onConversationLoad: async (conversation: Conversation) => {
    const { Stores } = await import('@/core/stores')

    // Sync model selection with conversation's model_id (use __state to avoid hooks)
    Stores.Chat.__state.ModelStore.initializeFromConversation(conversation.model_id)

    console.log(
      `[ModelExtension] Synced model selection for conversation: ${conversation.id}`,
    )
  },
})

export default modelExtension
