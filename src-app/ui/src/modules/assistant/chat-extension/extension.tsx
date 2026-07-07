import {
  createExtension,
  type ChatExtension,
  type ExtensionRequestFields,
} from '@/modules/chat/core/extensions'
import { Stores } from '@/core/stores'
import { AssistantMenuItem } from '@/modules/assistant/chat-extension/components/AssistantMenuItem'
import { AssistantStatusChip } from '@/modules/assistant/chat-extension/components/AssistantStatusChip'

/**
 * Assistant Extension (frontend chat-extension shim).
 *
 * Bridges the chat composer to the assistant module. The picker state
 * lives in modules/assistant/stores/AssistantPicker.store.ts
 * (registered as Stores.AssistantPicker), NOT under
 * Stores.Chat. This extension is a thin UI shim that:
 *   - Renders the toolbar picker + status chip components.
 *   - Reads the active picker selection into outgoing chat requests
 *     (composeRequestFields).
 *   - Resets/restores the picker selection when the chat conversation
 *     or editing-message changes (initialize subscriber).
 *
 * Auto-discovered by chat/extensions/index.ts via import.meta.glob
 * over '../../STAR/chat-extension/extension.tsx'.
 */
const assistantExtension: ChatExtension = createExtension({
  name: 'assistant',
  description: 'Provides assistant selection and configuration',
  priority: 80,

  initialize: async () => {
    const { useChatStore } = await import(
      '@/modules/chat/core/stores/Chat.store'
    )
    const { Stores } = await import('@/core/stores')

    // 1. Conversation-change → reset picker. Replaces the implicit
    //    chat-extension-framework scoping the old Stores.Chat.AssistantStore
    //    used to get for free; now we wire it explicitly because the
    //    store lives in the assistant module's namespace.
    useChatStore.subscribe(
      state => state.conversation?.id,
      () => {
        Stores.AssistantPicker.reset()
      },
    )

    // 2. Editing-message → restore the originally-attributed assistant.
    //    Save the pre-edit selection so we can restore it when the edit
    //    completes or is cancelled — never clear the user's choice.
    let preEditAssistantId: string | null = null

    useChatStore.subscribe(
      state => state.editingMessage,
      async (editingMessage) => {
        const picker = Stores.AssistantPicker
        if (!picker) return

        if (editingMessage) {
          // Save the assistant the user had selected before initiating
          // the edit, so we can restore it afterwards.
          preEditAssistantId = picker.$.selectedAssistantId

          // Per-message assistant attribution moved off the Message
          // row into the assistant bridge's own message_assistant
          // table (backend migration 75). Fetch via the assistant-
          // owned endpoint.
          try {
            const { ApiClient } = await import('@/api-client')
            const resp = await ApiClient.Message.getAssistant({
              id: editingMessage.id,
            })
            if (resp.assistant_id) {
              picker.selectAssistant(resp.assistant_id)
            } else {
              picker.clearAssistant()
            }
          } catch (err) {
            // Soft-fail: no attribution recorded (pre-migration
            // message or write hook failed at send-time) → keep
            // current assistant selection.
            preEditAssistantId = null
            console.warn(
              '[Assistant Extension] Failed to load message assistant attribution:',
              err,
            )
          }
        } else {
          // Edit cancelled or sent — restore the pre-edit selection
          // instead of blindly clearing.
          if (preEditAssistantId) {
            picker.selectAssistant(preEditAssistantId)
          } else {
            picker.clearAssistant()
          }
          preEditAssistantId = null
        }
      },
    )
  },

  slots: {
    toolbar_plus_items: { component: AssistantMenuItem, order: 30 },
    toolbar_status: { component: AssistantStatusChip, order: 20 },
  },

  composeRequestFields: async (): Promise<ExtensionRequestFields> => {
    // Read via `$` (hook-free snapshot) outside a React component context.
    const selectedAssistantId =
      Stores.AssistantPicker.$.selectedAssistantId

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
