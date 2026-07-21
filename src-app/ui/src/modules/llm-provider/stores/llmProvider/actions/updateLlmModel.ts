import { ApiClient } from '@/api-client'
import type { UpdateLlmModelRequest } from '@/api-client/types'
import type { LlmProviderGet, LlmProviderSet } from '../state'
import updateLlmModelInProviderFactory from './_updateLlmModelInProvider'

export default (set: LlmProviderSet, get: LlmProviderGet) => {
  const updateLlmModelInProvider = updateLlmModelInProviderFactory(set, get)
  return async (modelId: string, data: UpdateLlmModelRequest) => {
    const updated = await ApiClient.LlmModel.update({ model_id: modelId, ...data })
    const providerId = get().providers.find(p => p.llm_models?.some(m => m.id === modelId))?.id
    if (providerId) updateLlmModelInProvider(providerId, modelId, updated)
    return updated
  }
}
