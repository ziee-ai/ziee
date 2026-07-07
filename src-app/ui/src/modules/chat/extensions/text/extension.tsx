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
import type { MessageContent } from '@/api-client/types'

// The composer draft key captured at send START (before a new-chat conversation
// is created), so onMessageSent — which runs AFTER creation — clears exactly the
// key the text was authored under, never an unrelated conversation's draft.
let capturedDraftKey: string | null = null

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
    // right entry. `__state` (non-render access from an async hook).
    capturedDraftKey = makeDraftKey(
      Stores.Auth.__state.user?.id,
      Stores.Chat.__state.conversation?.id,
    )

    return { cancel: false }
  },

  /**
   * Clear text after message is sent
   * Called after message is successfully sent (before streaming starts)
   * Backup text before clearing for error recovery
   */
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    const textStore = Stores.Chat.$.TextStore

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
    const isBranchSend =
      Stores.Chat.__state.pendingBranchFromMessageId != null
    if (capturedDraftKey) {
      if (isBranchSend) {
        const draft = getDraft(capturedDraftKey)
        if (draft) textStore.setText(draft)
      } else {
        clearDraft(capturedDraftKey)
      }
    }
    capturedDraftKey = null
    console.log('[TextExtension] Backed up and cleared text after message sent')
    return {}
  },

  /**
   * Restore text on stream error
   * Called when streaming fails with an error
   */
  onStreamError: async (_error: Error) => {
    const { Stores } = await import('@/core/stores')
    const textStore = Stores.Chat.$.TextStore

    // Restore text from backup
    textStore.restoreFromBackup()
    console.log('[TextExtension] Restored text from backup after stream error')

    // Keep backup for potential retry (don't clear it yet)
    return {}
  },

  /**
   * Clear backup on successful completion
   * Called when streaming completes successfully
   */
  afterStreamComplete: async (_message) => {
    const { Stores } = await import('@/core/stores')
    const textStore = Stores.Chat.$.TextStore

    // Clear backup since message was sent successfully
    textStore.setBackupMessage(null)
    console.log('[TextExtension] Cleared text backup after successful stream')
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
