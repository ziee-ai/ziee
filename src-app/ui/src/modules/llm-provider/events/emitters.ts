import { Stores } from '@/core/stores'
import type { LlmProvider } from '@/api-client/types'

export const emitLlmProviderCreated = async (provider: LlmProvider) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.created',
    data: { provider },
  })
}

export const emitLlmProviderUpdated = async (provider: LlmProvider) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.updated',
    data: { provider },
  })
}

export const emitLlmProviderDeleted = async (providerId: string) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.deleted',
    data: { providerId },
  })
}

export const emitLlmModelEnabled = async (
  modelId: string,
  providerId: string,
) => {
  await Stores.EventBus.emit({
    type: 'llm_model.enabled',
    data: { modelId, providerId },
  })
}

export const emitLlmModelDisabled = async (
  modelId: string,
  providerId: string,
) => {
  await Stores.EventBus.emit({
    type: 'llm_model.disabled',
    data: { modelId, providerId },
  })
}

export const emitLlmModelDeleted = async (
  modelId: string,
  providerId: string,
) => {
  await Stores.EventBus.emit({
    type: 'llm_model.deleted',
    data: { modelId, providerId },
  })
}

export const emitLlmProviderGroupsChanged = async (
  providerId: string,
  groupIds: string[],
) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.groups_changed',
    data: { providerId, groupIds },
  })
}

export const emitGroupLlmProvidersChanged = async (
  groupId: string,
  providerIds: string[],
) => {
  await Stores.EventBus.emit({
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
  await Stores.EventBus.emit({
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
  await Stores.EventBus.emit({
    type: 'llm_model.download_failed',
    data: { downloadId, providerId, modelDisplayName, errorMessage },
  })
}
