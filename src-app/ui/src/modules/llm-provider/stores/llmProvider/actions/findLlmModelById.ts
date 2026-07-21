import type { LlmModel } from '@/api-client/types'
import type { LlmProviderGet } from '../state'

export default (_set: unknown, get: LlmProviderGet) =>
  (modelId: string): LlmModel | undefined => {
    for (const provider of get().providers) {
      const model = provider.llm_models?.find(m => m.id === modelId)
      if (model) return model
    }
    return undefined
  }
