import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { ModelSelector } from '@/modules/user-llm-providers/chat-extension/components/ModelSelector'
import type { Conversation } from '@/api-client/types'

/**
 * Model Extension (frontend chat-extension shim).
 *
 * Bridges the chat composer to the user-llm-providers module. The
 * picker state lives in modules/user-llm-providers/ModelPicker.store.ts
 * (registered as Stores.ModelPicker), NOT under Stores.Chat. This
 * extension is a thin UI shim that:
 *   - Renders the toolbar model selector slot.
 *   - Reads the active picker selection into outgoing chat requests
 *     (composeRequestFields).
 *   - Syncs the picker on conversation load AND on editing-message /
 *     conversation-id change (replaces the implicit auto-scoping the
 *     chat-extension-framework used to provide via createExtensionStore).
 *
 * Auto-discovered by chat/extensions/index.ts via the
 * import.meta.glob over '../../STAR/chat-extension/extension.tsx'.
 */
const modelExtension: ChatExtension = createExtension({
  name: 'model',
  description: 'Handles model selection for chat messages',
  priority: 10, // High priority - before text (5)

  initialize: async () => {
    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@ziee/framework/stores')

    // 1. Conversation-change → re-initialize the picker. Replaces
    //    the implicit chat-extension-framework scoping the old
    //    Stores.Chat.ModelStore used to get for free; now we wire it
    //    explicitly because the store lives in the user-llm-providers
    //    module's namespace.
    useChatStore.subscribe(
      state => state.conversation?.id,
      () => {
        const conversation = useChatStore.getState().conversation
        Stores.ModelPicker.initializeFromConversation(conversation?.model_id ?? undefined)
      },
    )

    // 2. Editing-message → restore the message's model id while
    //    editing, then fall back to the conversation default when the
    //    edit is cancelled or sent.
    useChatStore.subscribe(
      state => state.editingMessage,
      (editingMessage) => {
        const picker = Stores.ModelPicker
        if (!picker) return

        if (editingMessage?.model_id) {
          picker.setModelId(editingMessage.model_id)
        } else if (!editingMessage) {
          const conversation = useChatStore.getState().conversation
          picker.initializeFromConversation(conversation?.model_id ?? undefined)
        }
      },
    )
  },

  /**
   * Provide model_id to request.
   */
  composeRequestFields: async () => {
    const { Stores } = await import('@ziee/framework/stores')
    const modelId = Stores.ModelPicker.getModelId()
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
   * Sync model selection when conversation loads or switches. The
   * conversation-id subscriber above also handles this, but
   * onConversationLoad runs synchronously in the chat extension
   * lifecycle (the subscriber fires asynchronously after the store
   * commits) so keep both to avoid a one-frame stale picker.
   */
  onConversationLoad: async (conversation: Conversation) => {
    const { Stores } = await import('@ziee/framework/stores')
    Stores.ModelPicker.initializeFromConversation(conversation.model_id ?? undefined)
  },
})

export default modelExtension
