import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { ModelSelector } from '@/modules/user-llm-providers/chat-extension/components/ModelSelector'
import type { Conversation } from '@/api-client/types'
import { ModelPicker } from '@/modules/user-llm-providers/modelPicker'

/**
 * Model Extension (frontend chat-extension shim).
 *
 * Bridges the chat composer to the user-llm-providers module. The
 * picker state lives in modules/user-llm-providers/modelPicker/
 * (registered as ModelPicker), NOT under Chat. This
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
// Per-pane subscription teardown (ITEM-34/5), keyed by ctx.chatStore.
const paneModelSubs = new WeakMap<object, Array<() => void>>()

const modelExtension: ChatExtension = createExtension({
  name: 'model',
  description: 'Handles model selection for chat messages',
  priority: 10, // High priority - before text (5)

  initialize: async (ctx) => {
    const { newChatModelKey } = await import(
      '@/modules/user-llm-providers/modelPicker'
    )

    // Editing-message → restore the message's model id, then fall back to the
    // conversation default when the edit is cancelled/sent. Binds to the OWNING
    // pane's chat store (ctx.chatStore, ITEM-34/5) + keys by THAT pane's
    // conversation, so editing in a non-focused pane restores the right pane's
    // selection. (Per-conversation SEEDING is `onConversationLoad`, per pane.)
    const chatStore = ctx.chatStore
    const subs: Array<() => void> = []
    paneModelSubs.set(chatStore, subs)
    subs.push(
      chatStore.subscribe(
        (state: any) => state.editingMessage,
        (editingMessage: any) => {
          const conversation = chatStore.getState().conversation
          const key =
            conversation?.id ?? newChatModelKey(chatStore.getState().paneId)
          if (editingMessage?.model_id) {
            ModelPicker.setModelId(key, editingMessage.model_id)
          } else if (!editingMessage) {
            ModelPicker.initializeFromConversation(
              key,
              conversation?.model_id ?? undefined,
            )
          }
        },
      ),
    )
  },

  cleanup: async (ctx) => {
    const subs = paneModelSubs.get(ctx.chatStore)
    if (subs) {
      for (const unsub of subs) unsub()
      paneModelSubs.delete(ctx.chatStore)
    }
  },

  /**
   * Provide model_id to the request for the SENDING pane's conversation
   * (ctx.conversationId; null = new chat → the shared new-chat key). (ITEM-5)
   */
  composeRequestFields: async ctx => {
    const { newChatModelKey } = await import(
      '@/modules/user-llm-providers/modelPicker'
    )
    const key = ctx.conversationId ?? newChatModelKey(ctx.paneId)
    // getModelId/defaultModelId are LAZY store actions → they return a Promise
    // (the dispatcher loads the action chunk first). They MUST be awaited: using
    // the Promise directly makes `model_id` serialize to `{}`, so the backend
    // rejects the conversation-create with 422 "model_id: invalid type: map".
    const modelId =
      (await ModelPicker.getModelId(key)) ??
      (await ModelPicker.defaultModelId())
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
   * Seed the picker for THIS pane's conversation on load/switch — keyed by the
   * conversation id so each split pane keeps its own model selection (ITEM-5).
   * Runs per pane (each pane's loadConversation invokes it).
   */
  onConversationLoad: async (conversation: Conversation) => {
    ModelPicker.initializeFromConversation(
      conversation.id,
      conversation.model_id ?? undefined,
    )
  },
})

export default modelExtension
