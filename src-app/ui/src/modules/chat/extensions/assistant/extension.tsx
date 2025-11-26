import {
  createExtension,
  type ChatExtension,
  type ExtensionRequestFields,
} from '../../core/extensions'
import { Stores } from '@/core/stores'
import { createAssistantStore } from './AssistantStore.store'
import { AssistantSelector } from './components/AssistantSelector'

/**
 * Assistant Extension
 * Provides assistant selection in ChatInput toolbar
 * Allows users to switch between available assistants per-conversation
 */
const assistantExtension: ChatExtension = createExtension({
  name: 'AssistantStore',
  description: 'Provides assistant selection and configuration',
  priority: 80,

  // Create independent extension store
  createStore: createAssistantStore,

  // Lazy loading via __init__ pattern in slice - assistants load on first access

  // Register slot components
  slots: {
    toolbar_actions: { component: AssistantSelector, order: 80 },
  },

  composeRequestFields: async (): Promise<ExtensionRequestFields> => {
    // Add selected assistant ID to request if available
    // Use __state to access raw state outside React component context
    const selectedAssistantId =
      Stores.Chat.__state.AssistantStore.__state.selectedAssistantId

    if (selectedAssistantId) {
      return {
        assistant_id: selectedAssistantId,
      }
    }

    return {}
  },

  cleanup: async () => {
    console.log('[Assistant Extension] Cleaned up')
  },
})

export default assistantExtension
