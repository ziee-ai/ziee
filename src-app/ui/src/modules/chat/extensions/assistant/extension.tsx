import {
  createExtension,
  type ChatExtension,
  type ExtensionRequestFields,
} from '@/modules/chat/core/extensions'
import { Stores } from '@/core/stores'
import { createAssistantStore } from '@/modules/chat/extensions/assistant/Assistant.store'
import { AssistantMenuItem } from '@/modules/chat/extensions/assistant/components/AssistantMenuItem'
import { AssistantStatusChip } from '@/modules/chat/extensions/assistant/components/AssistantStatusChip'

/**
 * Assistant Extension
 * Provides assistant selection in ChatInput toolbar
 * Allows users to switch between available assistants per-conversation
 */
const assistantExtension: ChatExtension = createExtension({
  name: 'assistant',
  description: 'Provides assistant selection and configuration',
  priority: 80,

  // Create independent extension store
  store: {
    name: 'AssistantStore',
    createStore: createAssistantStore,
  },

  // Lazy loading via __init__ pattern in slice - assistants load on first access

  /**
   * Subscribe to editingMessage to restore assistant selection when editing a message
   */
  initialize: async () => {
    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@/core/stores')

    useChatStore.subscribe(
      state => state.editingMessage,
      async (editingMessage) => {
        const assistantStore = Stores.Chat.__state.AssistantStore
        if (!assistantStore) return

        if (editingMessage) {
          // Per-message assistant attribution moved off the Message
          // row into the assistant bridge's own `message_assistant`
          // table (backend migration 75). Fetch via the assistant-
          // owned endpoint instead of reading inline from
          // `editingMessage.assistant_id` (which no longer exists on
          // the Message type).
          try {
            const { ApiClient } = await import('@/api-client')
            const resp = await ApiClient.Message.getAssistant({
              id: editingMessage.id,
            })
            if (resp.assistant_id) {
              assistantStore.selectAssistant(resp.assistant_id)
            } else {
              assistantStore.clearAssistant()
            }
          } catch (err) {
            // Soft-fail: no attribution recorded (pre-migration
            // message or write hook failed at send-time) → keep
            // current assistant selection. Matches the pre-extraction
            // behavior for messages without the column populated.
            console.warn(
              '[Assistant Extension] Failed to load message assistant attribution:',
              err,
            )
          }
        } else {
          // Edit cancelled or sent — clear assistant selection
          assistantStore.clearAssistant()
        }
      }
    )
  },

  // Register slot components
  slots: {
    toolbar_plus_items: { component: AssistantMenuItem, order: 30 },
    toolbar_status: { component: AssistantStatusChip, order: 20 },
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
