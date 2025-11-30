import { createExtension, type ChatExtension } from '../../core/extensions'
import { createModelStore } from './Model.store'
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

    const selectedModel = Stores.Chat.__state.ModelStore.getModelId()
    if (!selectedModel) {
      throw new Error('No model selected')
    }

    const [, modelId] = selectedModel.split(':')

    return {
      model_id: modelId,
    }
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
