import type { BaseEvent } from '@ziee/framework/events'
import type { Assistant } from '@/api-client/types'

export interface AssistantCreatedEvent extends BaseEvent {
  type: 'assistant.created'
  data: {
    assistant: Assistant
  }
}

export interface AssistantUpdatedEvent extends BaseEvent {
  type: 'assistant.updated'
  data: {
    assistant: Assistant
  }
}

export interface AssistantDeletedEvent extends BaseEvent {
  type: 'assistant.deleted'
  data: {
    assistantId: string
  }
}

export interface AssistantTemplateCreatedEvent extends BaseEvent {
  type: 'assistant_template.created'
  data: {
    template: Assistant
  }
}

export interface AssistantTemplateUpdatedEvent extends BaseEvent {
  type: 'assistant_template.updated'
  data: {
    template: Assistant
  }
}

export interface AssistantTemplateDeletedEvent extends BaseEvent {
  type: 'assistant_template.deleted'
  data: {
    templateId: string
  }
}

export type AssistantModuleEvent =
  | AssistantCreatedEvent
  | AssistantUpdatedEvent
  | AssistantDeletedEvent
  | AssistantTemplateCreatedEvent
  | AssistantTemplateUpdatedEvent
  | AssistantTemplateDeletedEvent

declare module '@ziee/framework/events' {
  interface AppEvents {
    'assistant.created': AssistantCreatedEvent
    'assistant.updated': AssistantUpdatedEvent
    'assistant.deleted': AssistantDeletedEvent
    'assistant_template.created': AssistantTemplateCreatedEvent
    'assistant_template.updated': AssistantTemplateUpdatedEvent
    'assistant_template.deleted': AssistantTemplateDeletedEvent
  }
}
