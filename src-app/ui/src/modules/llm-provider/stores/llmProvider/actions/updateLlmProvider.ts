import { ApiClient } from '@/api-client'
import type { UpdateLlmProviderRequest } from '@/api-client/types'
import { emitLlmProviderUpdated } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'
import type { LlmProviderWithModels } from '../types'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (
    id: string,
    data: UpdateLlmProviderRequest,
  ): Promise<LlmProviderWithModels> => {
    const state = get()
    if (state.updating) return Promise.resolve(null as any)
    try {
      set({ updating: true, error: null })
      const provider = await ApiClient.LlmProvider.update({ provider_id: id, ...data })
      try {
        await emitLlmProviderUpdated(provider)
      } catch (eventError) {
        console.error('Failed to emit llm provider updated event:', eventError)
      }
      set({ updating: false })
      const existingProvider = state.providers.find(p => p.id === id)
      return { ...provider, llm_models: existingProvider?.llm_models || [] }
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to update provider',
        updating: false,
      })
      throw error
    }
  }
