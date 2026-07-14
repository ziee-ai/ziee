import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
  type StreamingContentProviders,
  type StreamingDeltaProcessors,
} from '@/modules/chat/core/extensions'
import { TextContent } from '@/modules/chat/extensions/text/components/TextContent'
import { ThinkingContent } from '@/modules/chat/extensions/text/components/ThinkingContent'
import { TextInput } from '@/modules/chat/extensions/text/components/TextInput'
import { createTextStore } from '@/modules/chat/extensions/text/Text.store'
import { clearDraft, getDraft, makeDraftKey } from '@/modules/chat/extensions/text/chatDrafts'
import { PaneDraftKeys } from '@/modules/chat/extensions/text/paneDraftKeys'
import type { MessageContent } from '@/api-client/types'

// The composer draft key captured at send START (before a new-chat conversation
// is created), so onMessageSent — which runs AFTER creation — clears exactly the
// key the text was authored under, never an unrelated conversation's draft.
//
// Keyed BY PANE (audit #4): a module-global `let` let two concurrent sends (or a
// send on a non-focused pane) clobber the key, and the async hooks read the
// FOCUSED pane's TextStore — which may have changed by the time streaming errors
// seconds later. Both are now pane-scoped: the key is stored under the sending
// pane's id (`PaneDraftKeys`, unit-tested for clobber-safety), and the hooks
// resolve the OWNING pane's TextStore via `ownerPaneId` (mirrors the File
// extension's ownerPaneId threading).
const capturedDraftKeys = new PaneDraftKeys()

/** The OWNING pane's Chat state (its own TextStore), not the focused-pane bridge.
 * `TextStore` is declaration-merged onto the `Stores.Chat` proxy, not the raw
 * store's `getState()` return, so we narrow to the fields this extension uses. */
async function ownerChatState(
  ownerPaneId?: string | null,
): Promise<{
  TextStore: ReturnType<typeof createTextStore>
  pendingBranchFromMessageId: string | null
}> {
  const { paneRegistry } = await import('@/modules/chat/core/stores/chatBridge')
  const { Chat } = await import('@/modules/chat/core/stores/Chat.store')
  const api = ownerPaneId ? paneRegistry.get(ownerPaneId)?.api : undefined
  return (api?.getState() ?? Chat.store.getState()) as unknown as {
    TextStore: ReturnType<typeof createTextStore>
    pendingBranchFromMessageId: string | null
  }
}

/**
 * Text Extension
 * Handles plain text and thinking content for messages
 */
const textExtension: ChatExtension = createExtension({
  name: 'text',
  description: 'Handles text and thinking content rendering and creation',
  priority: 5, // High priority - runs before file (80) and other extensions

  /**
   * Store for managing text input form instance
   */
  store: {
    name: 'TextStore',
    createStore: createTextStore,
  },

  /**
   * Provide user message content
   * Creates text content from user input
   * Uses text parameter passed from sendMessage (which reads from TextStore)
   */
  provideUserContent: async (text: string, _composedRequest: any): Promise<MessageContent[]> => {
    if (!text || text.trim() === '') {
      return []
    }

    const content: MessageContent = {
      id: crypto.randomUUID(),
      message_id: '', // Will be set by backend
      content_type: 'text',
      content: { type: 'text', text: text.trim() },
      sequence_order: 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }

    return [content]
  },

  /**
   * Type-safe streaming content providers
   * Creates initial content blocks for text/thinking during streaming
   * Uses registry-based O(1) lookup instead of if/else chains
   */
  streamingContentProviders: {
    text: (delta) => ({
      id: crypto.randomUUID(),
      message_id: '', // Will be set by Chat store
      content_type: 'text',
      content: { type: 'text', text: delta || '' },
      sequence_order: 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }),

    thinking: (delta) => ({
      id: crypto.randomUUID(),
      message_id: '',
      content_type: 'thinking',
      content: { type: 'thinking', thinking: delta || '', metadata: null },
      sequence_order: 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }),
  } satisfies StreamingContentProviders,

  /**
   * Type-safe streaming delta processors
   * Accumulates text/thinking deltas using registry-based O(1) lookup
   */
  streamingDeltaProcessors: {
    text: (content, delta) => {
      // content.content is automatically typed as MessageContentDataText - no casting needed!
      return {
        ...content,
        content: {
          ...content.content,
          text: content.content.text + delta,
        },
        updated_at: new Date().toISOString(),
      }
    },

    thinking: (content, delta) => {
      // content.content is automatically typed as MessageContentDataThinking - no casting needed!
      return {
        ...content,
        content: {
          ...content.content,
          thinking: content.content.thinking + delta,
        },
        updated_at: new Date().toISOString(),
      }
    },
  } satisfies StreamingDeltaProcessors,

  /**
   * Register content type renderers
   */
  contentTypes: {
    text: TextContent as React.ComponentType<ContentRendererProps>,
    thinking: ThinkingContent as React.ComponentType<ContentRendererProps>,
  },

  /**
   * Compose request fields
   * Returns content field with text from TextStore
   */
  composeRequestFields: async () => {
    const { Stores } = await import('@/core/stores')
    const content = Stores.Chat.$.TextStore.getText()
    return { content: content?.trim() || '' }
  },

  /**
   * Validate text before sending
   * Only validates - does not return message (that's handled by composeRequestFields)
   */
  beforeSendMessage: async () => {
    const { Stores } = await import('@/core/stores')
    const content = Stores.Chat.$.TextStore.getText()

    if (!content || !content.trim()) {
      return {
        cancel: true,
        errorMessage: 'Message cannot be empty',
      }
    }

    // Capture the draft key NOW, before a new-chat send creates the conversation
    // (which would flip the composer's key to the new id). For an existing
    // conversation it's that id; for a new chat it's the shared `new` bucket.
    // Must match TextInput's user-namespaced draftKey so onMessageSent clears the
    // right entry. Read via `$` snapshot (non-render access from an async hook).
    // Keyed by the SENDING pane (= focused at send start, ITEM-33) so a concurrent
    // send on another pane can't clobber it; onMessageSent reads it back by the
    // owning pane's id (which equals this focused id).
    capturedDraftKeys.set(
      Stores.SplitView.$.focusedPaneId,
      makeDraftKey(Stores.Auth.$.user?.id, Stores.Chat.$.conversation?.id),
    )

    return { cancel: false }
  },

  /**
   * Clear text after message is sent
   * Called after message is successfully sent (before streaming starts)
   * Backup text before clearing for error recovery
   */
  onMessageSent: async (ownerPaneId) => {
    // Act on the OWNING pane's TextStore/state (audit #4), not the focused-pane
    // bridge — the sending pane may no longer be focused by the time this fires.
    const state = await ownerChatState(ownerPaneId)
    const textStore = state.TextStore

    // Backup text before clearing
    const currentText = textStore.getText()
    textStore.setBackupMessage(currentText)

    // Clear the visible composer text.
    textStore.clearText()

    // A normal send's composer text IS the draft → clear the persisted draft
    // (the captured pre-creation key). An edit/regen submit instead OVERWROTE
    // the composer via programmatic setText, so the user's real unsent draft
    // still sits in localStorage — restore it into the now-cleared composer.
    // (onMessageSent fires while pendingBranchFromMessageId is still set —
    // cleared right after — so we can tell the two apart. Restoring here also
    // covers regenerate, which sets no editingMessage and so wouldn't trigger
    // the TextInput restore-on-edit-end effect.)
    const isBranchSend = state.pendingBranchFromMessageId != null
    const capturedDraftKey = capturedDraftKeys.take(ownerPaneId)
    if (capturedDraftKey) {
      if (isBranchSend) {
        const draft = getDraft(capturedDraftKey)
        if (draft) textStore.setText(draft)
      } else {
        clearDraft(capturedDraftKey)
      }
    }
    return {}
  },

  /**
   * Restore text on stream error
   * Called when streaming fails with an error
   */
  onStreamError: async (_error: Error, ownerPaneId) => {
    // Restore into the OWNING pane's composer (audit #4), not the focused one.
    const textStore = (await ownerChatState(ownerPaneId)).TextStore

    // Restore text from backup
    textStore.restoreFromBackup()

    // Keep backup for potential retry (don't clear it yet)
    return {}
  },

  /**
   * Clear backup on successful completion
   * Called when streaming completes successfully
   */
  afterStreamComplete: async (_message, ownerPaneId) => {
    // Clear the OWNING pane's backup (audit #4), not the focused one.
    const textStore = (await ownerChatState(ownerPaneId)).TextStore

    // Clear backup since message was sent successfully
    textStore.setBackupMessage(null)
    return {}
  },

  /**
   * Register text input component
   */
  slots: {
    text_input: { component: TextInput, order: 0 },
  },
})

export default textExtension
