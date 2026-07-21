import { ApiClient } from '@/api-client'
import type { CreateLlmProviderRequest } from '@/api-client/types'
import { emitLlmProviderCreated } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'
import type { LlmProviderWithModels } from '../types'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (data: CreateLlmProviderRequest): Promise<LlmProviderWithModels> => {
    if (get().creating) return Promise.resolve(null as any)
    try {
      set({ creating: true, error: null })
      const provider = await ApiClient.LlmProvider.create(data)
      try {
        await emitLlmProviderCreated(provider)
      } catch (eventError) {
        console.error('Failed to emit llm provider created event:', eventError)
      }
      set({ creating: false })
      return { ...provider, llm_models: [] }
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create provider',
        creating: false,
      })
      throw error
    }
  }
