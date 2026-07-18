import type { BaseEvent } from '@ziee/framework/events'
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
 * Emitted when a conversation is deleted. Subscribers drop the row
 * from their local state. Closes audit F5: previously delete didn't
 * propagate, so widgets that maintained their own fetch could stay
 * stale.
 */
export interface ConversationDeletedEvent extends BaseEvent {
  type: 'conversation.deleted'
  data: {
    conversationId: string
  }
}

/**
 * Augment global AppEvents registry
 */
declare module '@ziee/framework/events' {
  interface AppEvents {
    'conversation.created': ConversationCreatedEvent
    'conversation.titleUpdated': ConversationTitleUpdatedEvent
    'conversation.deleted': ConversationDeletedEvent
  }
}

export {}
