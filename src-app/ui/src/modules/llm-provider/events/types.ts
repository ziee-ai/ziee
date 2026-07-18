import type { BaseEvent } from '@ziee/framework/events'
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

/**
 * Emitted by `LlmModelDownload.store.ts`'s SSE handler the moment a
 * download row transitions to `status === 'completed'`. Fired EXACTLY
 * ONCE per download — the handler keeps a pre-update status snapshot
 * so subsequent broadcasts for the same already-completed row don't
 * re-fire. Consumed by the globally-mounted
 * `LlmModelDownloadNotifications` listener to surface a success toast,
 * even when the user has navigated away from the hub page.
 */
export interface LlmModelDownloadCompletedEvent extends BaseEvent {
  type: 'llm_model.download_completed'
  data: {
    downloadId: string
    providerId: string
    modelDisplayName: string
  }
}

/**
 * Sibling of `LlmModelDownloadCompletedEvent` for terminal failures.
 * Same emit-once semantics. `errorMessage` carries the backend's
 * human-readable reason verbatim — the listener surfaces it in the
 * toast so the user sees what went wrong without digging into the
 * provider settings page.
 */
export interface LlmModelDownloadFailedEvent extends BaseEvent {
  type: 'llm_model.download_failed'
  data: {
    downloadId: string
    providerId: string
    modelDisplayName: string
    errorMessage: string
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
  | LlmModelDownloadCompletedEvent
  | LlmModelDownloadFailedEvent

declare module '@ziee/framework/events' {
  interface AppEvents {
    'llm_provider.created': LlmProviderCreatedEvent
    'llm_provider.updated': LlmProviderUpdatedEvent
    'llm_provider.deleted': LlmProviderDeletedEvent
    'llm_model.enabled': LlmModelEnabledEvent
    'llm_model.disabled': LlmModelDisabledEvent
    'llm_model.deleted': LlmModelDeletedEvent
    'llm_provider.groups_changed': LlmProviderGroupsChangedEvent
    'llm_provider.group_providers_changed': GroupLlmProvidersChangedEvent
    'llm_model.download_completed': LlmModelDownloadCompletedEvent
    'llm_model.download_failed': LlmModelDownloadFailedEvent
  }
}
