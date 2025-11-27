import type { BaseEvent } from '@/core/events'
import type { Conversation } from '@/api-client/types'

/**
 * Chat module events
 * These events are emitted by the Chat store and extensions
 */

/**
 * Emitted when a new conversation is created
 */
export interface ConversationCreatedEvent extends BaseEvent {
  type: 'conversation.created'
  data: {
    conversation: Conversation
  }
}

/**
 * Emitted when a conversation title is updated
 */
export interface ConversationTitleUpdatedEvent extends BaseEvent {
  type: 'conversation.titleUpdated'
  data: {
    conversationId: string
    title: string
  }
}

/**
 * Augment global AppEvents registry
 */
declare module '@/core/events' {
  interface AppEvents {
    'conversation.created': ConversationCreatedEvent
    'conversation.titleUpdated': ConversationTitleUpdatedEvent
  }
}

export {}
