import { ApiClient } from '@/api-client'
import { emitLlmModelDeleted } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (modelId: string) => {
    try {
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
        error: null,
      }))
      const providerId = get().providers.find(p =>
        p.llm_models?.some(m => m.id === modelId),
      )?.id
      await ApiClient.LlmModel.delete({ model_id: modelId })
      if (providerId) {
        try {
          await emitLlmModelDeleted(modelId, providerId)
        } catch (eventError) {
          console.error('Failed to emit llm model deleted event:', eventError)
        }
      }
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
    } catch (error) {
      set(state => ({
        error: error instanceof Error ? error.message : 'Failed to delete model',
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
      throw error
    }
  }
