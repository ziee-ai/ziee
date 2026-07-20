import {
  createExtension,
  type ChatExtension,
  type ExtensionRequestFields,
} from '@/modules/chat/core/extensions'
import { Stores } from '@ziee/framework/stores'
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
// Per-pane subscription teardown (ITEM-34/5), keyed by ctx.chatStore.
const paneAssistantSubs = new WeakMap<object, Array<() => void>>()

const assistantExtension: ChatExtension = createExtension({
  name: 'assistant',
  description: 'Provides assistant selection and configuration',
  priority: 80,

  initialize: async (ctx) => {
    const { Stores } = await import('@ziee/framework/stores')
    const { newChatAssistantKey } = await import(
      '@/modules/assistant/stores/AssistantPicker.store'
    )

    // Per-conversation keying makes the old "reset on conversation change"
    // subscription unnecessary — a conversation with no map entry simply has no
    // assistant (ITEM-5). The editing-message restore binds to the OWNING pane's
    // chat store (ctx.chatStore, ITEM-34/5) + keys by THAT pane's conversation,
    // so editing in a non-focused pane restores the right pane's selection.
    let preEditAssistantId: string | null = null
    const chatStore = ctx.chatStore
    const paneKey = () =>
      chatStore.getState().conversation?.id ??
      newChatAssistantKey(chatStore.getState().paneId)
    const subs: Array<() => void> = []
    paneAssistantSubs.set(chatStore, subs)

    subs.push(
      chatStore.subscribe(
        (state: any) => state.editingMessage,
        async (editingMessage: any) => {
        const picker = Stores.AssistantPicker
        if (!picker) return
        const key = paneKey()

        if (editingMessage) {
          // Save the assistant the user had selected before initiating the edit.
          preEditAssistantId = picker.getAssistantId(key)

          // Per-message assistant attribution moved off the Message row into the
          // assistant bridge's own message_assistant table (backend migration
          // 75). Fetch via the assistant-owned endpoint.
          try {
            const { ApiClient } = await import('@/api-client')
            const resp = await ApiClient.Message.getAssistant({
              id: editingMessage.id,
            })
            if (resp.assistant_id) {
              picker.selectAssistant(key, resp.assistant_id)
            } else {
              picker.clearAssistant(key)
            }
          } catch (err) {
            // Soft-fail: no attribution recorded → keep current selection.
            preEditAssistantId = null
            console.warn(
              '[Assistant Extension] Failed to load message assistant attribution:',
              err,
            )
          }
        } else {
          // Edit cancelled or sent — restore the pre-edit selection.
          if (preEditAssistantId) {
            picker.selectAssistant(key, preEditAssistantId)
          } else {
            picker.clearAssistant(key)
          }
          preEditAssistantId = null
        }
        },
      ),
    )
  },

  slots: {
    toolbar_plus_items: { component: AssistantMenuItem, order: 30 },
    toolbar_status: { component: AssistantStatusChip, order: 20 },
  },

  composeRequestFields: async (ctx): Promise<ExtensionRequestFields> => {
    // The SENDING pane's assistant (ctx.conversationId; null = new chat). (ITEM-5)
    const { newChatAssistantKey } = await import(
      '@/modules/assistant/stores/AssistantPicker.store'
    )
    const key = ctx.conversationId ?? newChatAssistantKey(ctx.paneId)
    // Effective (not raw) so an untouched new chat auto-sends the user's default
    // (`is_default`) assistant; an explicit clear (null) still sends nothing.
    const selectedAssistantId =
      Stores.AssistantPicker.getEffectiveAssistantId(key)

    if (selectedAssistantId) {
      return {
        assistant_id: selectedAssistantId,
      }
    }

    return {}
  },

  cleanup: async (ctx) => {
    const subs = paneAssistantSubs.get(ctx.chatStore)
    if (subs) {
      for (const unsub of subs) unsub()
      paneAssistantSubs.delete(ctx.chatStore)
    }
  },
})

export default assistantExtension
