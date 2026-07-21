import type { BaseEvent } from '@ziee/framework/events'
import type { LlmProvider } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitLlmProviderCreated = async (provider: LlmProvider) => {
  await EventBus.emit({
    type: 'llm_provider.created',
    data: { provider },
  })
}

export const emitLlmProviderUpdated = async (provider: LlmProvider) => {
  await EventBus.emit({
    type: 'llm_provider.updated',
    data: { provider },
  })
}

export const emitLlmProviderDeleted = async (providerId: string) => {
  await EventBus.emit({
    type: 'llm_provider.deleted',
    data: { providerId },
  })
}

export const emitLlmModelEnabled = async (
  modelId: string,
  providerId: string,
) => {
  await EventBus.emit({
    type: 'llm_model.enabled',
    data: { modelId, providerId },
  })
}

export const emitLlmModelDisabled = async (
  modelId: string,
  providerId: string,
) => {
  await EventBus.emit({
    type: 'llm_model.disabled',
    data: { modelId, providerId },
  })
}

export const emitLlmModelDeleted = async (
  modelId: string,
  providerId: string,
) => {
  await EventBus.emit({
    type: 'llm_model.deleted',
    data: { modelId, providerId },
  })
}

export const emitLlmProviderGroupsChanged = async (
  providerId: string,
  groupIds: string[],
) => {
  await EventBus.emit({
    type: 'llm_provider.groups_changed',
    data: { providerId, groupIds },
  })
}

export const emitGroupLlmProvidersChanged = async (
  groupId: string,
  providerIds: string[],
) => {
  await EventBus.emit({
    type: 'llm_provider.group_providers_changed',
    data: { groupId, providerIds },
  })
}

/**
 * Fired from `LlmModelDownload.store.ts` on the SSE tick where a row
 * transitions to `status === 'completed'`. The
 * `LlmModelDownloadNotifications` listener surfaces a green toast.
 */
export const emitLlmModelDownloadCompleted = async (
  downloadId: string,
  providerId: string,
  modelDisplayName: string,
) => {
  await EventBus.emit({
    type: 'llm_model.download_completed',
    data: { downloadId, providerId, modelDisplayName },
  })
}

/**
 * Sibling of completed — fired on the transition to
 * `status === 'failed'`. The listener surfaces a red toast carrying
 * `errorMessage` so the user sees the backend reason without digging
 * into provider settings.
 */
export const emitLlmModelDownloadFailed = async (
  downloadId: string,
  providerId: string,
  modelDisplayName: string,
  errorMessage: string,
) => {
  await EventBus.emit({
    type: 'llm_model.download_failed',
    data: { downloadId, providerId, modelDisplayName, errorMessage },
  })
}

// --- User API key events ---

export interface ApiKeySavedEvent extends BaseEvent {
  type: 'api_key.saved'
  data: { providerId: string }
}

export interface ApiKeyDeletedEvent extends BaseEvent {
  type: 'api_key.deleted'
  data: { providerId: string }
}

declare module '@ziee/framework/events' {
  interface AppEvents {
    'api_key.saved': ApiKeySavedEvent
    'api_key.deleted': ApiKeyDeletedEvent
  }
}

/**
 * Fired from `UserProviderKeys.store.ts` after a key is successfully
 * saved. Also triggers a reload in `UserLlmProvidersStore` so both
 * stores stay in sync.
 */
export const emitApiKeySaved = async (providerId: string) => {
  await EventBus.emit({
    type: 'api_key.saved',
    data: { providerId },
  })
}

/**
 * Fired from `UserProviderKeys.store.ts` after a key is successfully
 * deleted. Also triggers a reload in `UserLlmProvidersStore` so both
 * stores stay in sync.
 */
export const emitApiKeyDeleted = async (providerId: string) => {
  await EventBus.emit({
    type: 'api_key.deleted',
    data: { providerId },
  })
}
