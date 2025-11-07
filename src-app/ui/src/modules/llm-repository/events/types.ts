import type { BaseEvent } from '@/core/events'
import type { LlmRepository } from '@/api-client/types'

export interface LlmRepositoryCreatedEvent extends BaseEvent {
  type: 'llm_repository.created'
  data: {
    repository: LlmRepository
  }
}

export interface LlmRepositoryUpdatedEvent extends BaseEvent {
  type: 'llm_repository.updated'
  data: {
    repository: LlmRepository
  }
}

export interface LlmRepositoryDeletedEvent extends BaseEvent {
  type: 'llm_repository.deleted'
  data: {
    repositoryId: string
  }
}

export type LlmRepositoryModuleEvent =
  | LlmRepositoryCreatedEvent
  | LlmRepositoryUpdatedEvent
  | LlmRepositoryDeletedEvent

declare module '@/core/events' {
  interface AppEvents {
    'llm_repository.created': LlmRepositoryCreatedEvent
    'llm_repository.updated': LlmRepositoryUpdatedEvent
    'llm_repository.deleted': LlmRepositoryDeletedEvent
  }
}
