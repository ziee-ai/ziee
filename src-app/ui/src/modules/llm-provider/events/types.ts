import type { BaseEvent } from '@/core/events'
import type { LlmProvider } from '@/api-client/types'

export interface LlmProviderCreatedEvent extends BaseEvent {
  type: 'llm_provider.created'
  data: {
    provider: LlmProvider
  }
}

export interface LlmProviderUpdatedEvent extends BaseEvent {
  type: 'llm_provider.updated'
  data: {
    provider: LlmProvider
  }
}

export interface LlmProviderDeletedEvent extends BaseEvent {
  type: 'llm_provider.deleted'
  data: {
    providerId: string
  }
}

export interface LlmModelEnabledEvent extends BaseEvent {
  type: 'llm_model.enabled'
  data: {
    modelId: string
    providerId: string
  }
}

export interface LlmModelDisabledEvent extends BaseEvent {
  type: 'llm_model.disabled'
  data: {
    modelId: string
    providerId: string
  }
}

export interface LlmModelDeletedEvent extends BaseEvent {
  type: 'llm_model.deleted'
  data: {
    modelId: string
    providerId: string
  }
}

export interface LlmProviderGroupsChangedEvent extends BaseEvent {
  type: 'llm_provider.groups_changed'
  data: {
    providerId: string
    groupIds: string[]
  }
}

export interface GroupLlmProvidersChangedEvent extends BaseEvent {
  type: 'llm_provider.group_providers_changed'
  data: {
    groupId: string
    providerIds: string[]
  }
}

export type LlmProviderModuleEvent =
  | LlmProviderCreatedEvent
  | LlmProviderUpdatedEvent
  | LlmProviderDeletedEvent
  | LlmModelEnabledEvent
  | LlmModelDisabledEvent
  | LlmModelDeletedEvent
  | LlmProviderGroupsChangedEvent
  | GroupLlmProvidersChangedEvent

declare module '@/core/events' {
  interface AppEvents {
    'llm_provider.created': LlmProviderCreatedEvent
    'llm_provider.updated': LlmProviderUpdatedEvent
    'llm_provider.deleted': LlmProviderDeletedEvent
    'llm_model.enabled': LlmModelEnabledEvent
    'llm_model.disabled': LlmModelDisabledEvent
    'llm_model.deleted': LlmModelDeletedEvent
    'llm_provider.groups_changed': LlmProviderGroupsChangedEvent
    'llm_provider.group_providers_changed': GroupLlmProvidersChangedEvent
  }
}
