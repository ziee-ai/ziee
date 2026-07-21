import { ApiClient } from '@/api-client'
import { emitLlmModelDisabled } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (modelId: string) => {
    try {
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
        error: null,
      }))
      const model = await ApiClient.LlmModel.update({ model_id: modelId, enabled: false })
      const providerId = get().providers.find(p =>
        p.llm_models?.some(m => m.id === modelId),
      )?.id
      if (providerId) {
        try {
          await emitLlmModelDisabled(modelId, providerId)
        } catch (eventError) {
          console.error('Failed to emit llm model disabled event:', eventError)
        }
      }
      set(state => ({
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
      return model
    } catch (error) {
      set(state => ({
        error: error instanceof Error ? error.message : 'Failed to disable model',
        llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
      }))
      throw error
    }
  }
