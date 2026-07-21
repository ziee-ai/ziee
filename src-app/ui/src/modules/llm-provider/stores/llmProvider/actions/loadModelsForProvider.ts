import { ApiClient } from '@/api-client'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (providerId: string) => {
    // Concurrent-load dedup: skip if a load is already in flight for this
    // provider (SSE handler + user click can race). (audit 05 H-4)
    if (get().llmModelsLoading[providerId]) return
    try {
      set(state => ({
        llmModelsLoading: { ...state.llmModelsLoading, [providerId]: true },
        modelError: { ...state.modelError, [providerId]: '' },
      }))
      const modelsResponse = await ApiClient.LlmModel.list({
        providerId,
        page: 1,
        perPage: 100,
      })
      set(state => ({
        providers: state.providers.map(p =>
          p.id === providerId ? { ...p, llm_models: modelsResponse.models } : p,
        ),
        llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
      }))
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to load models'
      console.error(`Failed to load models for provider ${providerId}:`, error)
      set(state => ({
        llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
        modelError: { ...state.modelError, [providerId]: errorMessage },
      }))
    }
  }
