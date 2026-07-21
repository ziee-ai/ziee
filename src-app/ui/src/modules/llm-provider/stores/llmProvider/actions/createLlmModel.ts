import { ApiClient } from '@/api-client'
import type { CreateLlmModelRequest } from '@/api-client/types'
import type { LlmProviderGet, LlmProviderSet } from '../state'
import addLlmModelToProviderFactory from './_addLlmModelToProvider'
import loadLlmProvidersFactory from './loadLlmProviders'

export default (set: LlmProviderSet, get: LlmProviderGet) => {
  const addLlmModelToProvider = addLlmModelToProviderFactory(set, get)
  const loadLlmProviders = loadLlmProvidersFactory(set, get)
  return async (
    providerId: string,
    data: Omit<CreateLlmModelRequest, 'provider_id'>,
  ) => {
    const model = await ApiClient.LlmModel.create({ ...data, provider_id: providerId })
    // Optimistically append, then refresh so backend enrichment shows.
    addLlmModelToProvider(providerId, model)
    await loadLlmProviders()
    return model
  }
}
