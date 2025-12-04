import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
  type StreamingContentProviders,
  type StreamingDeltaProcessors,
} from '../../core/extensions'
import { TextContent } from './components/TextContent'
import { ThinkingContent } from './components/ThinkingContent'
import { TextInput } from './components/TextInput'
import { createTextStore } from './Text.store'
import type { MessageContent } from '@/api-client/types'

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
    const content = Stores.Chat.__state.TextStore.getText()
    return { content: content?.trim() || '' }
  },

  /**
   * Validate text before sending
   * Only validates - does not return message (that's handled by composeRequestFields)
   */
  beforeSendMessage: async () => {
    const { Stores } = await import('@/core/stores')
    const content = Stores.Chat.__state.TextStore.getText()

    if (!content || !content.trim()) {
      return {
        cancel: true,
        errorMessage: 'Message cannot be empty',
      }
    }

    return { cancel: false }
  },

  /**
   * Clear text after message is sent
   * Called after message is successfully sent (before streaming starts)
   * Backup text before clearing for error recovery
   */
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    const textStore = Stores.Chat.__state.TextStore

    // Backup text before clearing
    const currentText = textStore.getText()
    textStore.setBackupMessage(currentText)

    // Clear text
    textStore.clearText()
    console.log('[TextExtension] Backed up and cleared text after message sent')
    return {}
  },

  /**
   * Restore text on stream error
   * Called when streaming fails with an error
   */
  onStreamError: async (_error: Error) => {
    const { Stores } = await import('@/core/stores')
    const textStore = Stores.Chat.__state.TextStore

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
    const textStore = Stores.Chat.__state.TextStore

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
