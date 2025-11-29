import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '../../core/extensions'
import { TextContent } from './components/TextContent'
import { ThinkingContent } from './components/ThinkingContent'
import type { MessageContent } from '@/api-client/types'

/**
 * Text Extension
 * Handles plain text and thinking content for messages
 */
const textExtension: ChatExtension = createExtension({
  name: 'TextExtension',
  description: 'Handles text and thinking content rendering and creation',
  priority: 5, // High priority - runs before file (80) and other extensions

  /**
   * Provide user message content
   * Creates text content from user input
   */
  provideUserContent: async (text: string, _composedRequest: any): Promise<MessageContent[]> => {
    if (!text || text.trim() === '') {
      return []
    }

    const content: MessageContent = {
      id: crypto.randomUUID(),
      message_id: '', // Will be set by backend
      content_type: 'text',
      content: { type: 'text', text },
      sequence_order: 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }

    return [content]
  },

  /**
   * Provide streaming content
   * Creates new Text or Thinking content blocks during streaming
   */
  provideStreamingContent: async (
    contentType: string,
    delta?: string,
  ): Promise<MessageContent | null> => {
    if (contentType === 'text') {
      return {
        id: crypto.randomUUID(),
        message_id: '', // Will be set by Chat store
        content_type: 'text',
        content: { type: 'text', text: delta || '' },
        sequence_order: 0,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }
    }

    if (contentType === 'thinking') {
      return {
        id: crypto.randomUUID(),
        message_id: '',
        content_type: 'thinking',
        content: { type: 'thinking', thinking: delta || '', metadata: null },
        sequence_order: 0,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }
    }

    return null
  },

  /**
   * Process streaming delta
   * Accumulates text/thinking deltas
   */
  processStreamingDelta: async (
    content: MessageContent,
    delta: string,
  ): Promise<MessageContent> => {
    const contentData = content.content as any

    if (contentData.type === 'text') {
      return {
        ...content,
        content: {
          ...contentData,
          text: contentData.text + delta,
        },
        updated_at: new Date().toISOString(),
      }
    }

    if (contentData.type === 'thinking') {
      return {
        ...content,
        content: {
          ...contentData,
          thinking: contentData.thinking + delta,
        },
        updated_at: new Date().toISOString(),
      }
    }

    // Not our content type - return unchanged
    return content
  },

  /**
   * Register content type renderers
   */
  contentTypes: {
    text: TextContent as React.ComponentType<ContentRendererProps>,
    thinking: ThinkingContent as React.ComponentType<ContentRendererProps>,
  },
})

export default textExtension
