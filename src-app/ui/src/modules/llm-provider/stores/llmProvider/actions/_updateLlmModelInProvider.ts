import type { LlmModel } from '@/api-client/types'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, _get: LlmProviderGet) =>
  (providerId: string, modelId: string, updatedModel: LlmModel) => {
    set(state => ({
      providers: state.providers.map(p =>
        p.id === providerId
          ? { ...p, llm_models: p.llm_models?.map(m => (m.id === modelId ? updatedModel : m)) }
          : p,
      ),
    }))
  }
