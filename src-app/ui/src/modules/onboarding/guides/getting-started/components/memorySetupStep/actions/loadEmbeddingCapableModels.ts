import { ApiClient } from '@/api-client'
import type { EmbeddingCapableModel } from '../state'
import type { MemorySetupStepGet, MemorySetupStepSet } from '../state'

export default (set: MemorySetupStepSet, _get: MemorySetupStepGet) =>
  async () => {
    set(draft => {
      draft.loading = true
      draft.error = null
    })
    try {
      // Server-side filter `?capability=text_embedding` on the typed endpoint.
      const body = await ApiClient.LlmModel.list({
        capability: 'text_embedding',
        page: 1,
        perPage: 200,
      })
      const models: EmbeddingCapableModel[] = body.models.map(m => ({
        id: m.id,
        name: m.name,
        display_name: m.display_name,
        provider_id: m.provider_id,
      }))
      set(draft => {
        draft.availableModels = models
        draft.loading = false
      })
    } catch (e: any) {
      set(draft => {
        draft.error = e?.message || 'Failed to load embedding-capable models'
        draft.loading = false
      })
    }
  }
