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

/**
 * Emitted by the backend's `connection_health::enforce_on_*` paths
 * when a repository was flipped to `enabled = false` because its
 * probe failed. The store listens to reload the list so the row's
 * `last_health_check_status` flips to 'unhealthy' in the DOM
 * without a manual refresh. Boot-time auto-disables do NOT emit
 * this — the EventBus isn't built yet at module init; mount-time
 * refetch on the settings page catches them.
 */
export interface LlmRepositoryAutoDisabledEvent extends BaseEvent {
  type: 'llm_repository.auto_disabled'
  data: {
    repositoryId: string
    reason: string
  }
}

export type LlmRepositoryModuleEvent =
  | LlmRepositoryCreatedEvent
  | LlmRepositoryUpdatedEvent
  | LlmRepositoryDeletedEvent
  | LlmRepositoryAutoDisabledEvent

declare module '@/core/events' {
  interface AppEvents {
    'llm_repository.created': LlmRepositoryCreatedEvent
    'llm_repository.updated': LlmRepositoryUpdatedEvent
    'llm_repository.deleted': LlmRepositoryDeletedEvent
    'llm_repository.auto_disabled': LlmRepositoryAutoDisabledEvent
  }
}
