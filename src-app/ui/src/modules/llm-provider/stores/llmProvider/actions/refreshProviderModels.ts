import { ApiClient } from '@/api-client'
import type { LlmModel } from '@/api-client/types'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, _get: LlmProviderGet) =>
  async (providerId: string): Promise<LlmModel[]> => {
    set(state => ({
      refreshingModels: { ...state.refreshingModels, [providerId]: true },
    }))
    try {
      const models = await ApiClient.LlmProvider.refreshModels({ provider_id: providerId })
      set(state => ({
        providers: state.providers.map(p =>
          p.id === providerId ? { ...p, llm_models: models } : p,
        ),
        refreshingModels: { ...state.refreshingModels, [providerId]: false },
      }))
      return models
    } catch (error) {
      set(state => ({
        error: error instanceof Error ? error.message : 'Failed to refresh models',
        refreshingModels: { ...state.refreshingModels, [providerId]: false },
      }))
      throw error
    }
  }
