import { ApiClient } from '@/api-client'
import { emitLlmModelEnabled } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (modelId: string) => {
    try {
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
        error: null,
      }))
      const model = await ApiClient.LlmModel.update({ model_id: modelId, enabled: true })
      const providerId = get().providers.find(p =>
        p.llm_models?.some(m => m.id === modelId),
      )?.id
      if (providerId) {
        try {
          await emitLlmModelEnabled(modelId, providerId)
        } catch (eventError) {
          console.error('Failed to emit llm model enabled event:', eventError)
        }
      }
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
      return model
    } catch (error) {
      set(state => ({
        error: error instanceof Error ? error.message : 'Failed to enable model',
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
      throw error
    }
  }
