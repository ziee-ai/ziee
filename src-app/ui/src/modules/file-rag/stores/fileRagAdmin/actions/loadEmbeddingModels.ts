import { ApiClient } from '@/api-client'
import type { FileRagAdminSet } from '../state'
import toRow from './_toRow'

export default (set: FileRagAdminSet) =>
  async () => {
    set(s => {
      s.loadingModels = true
    })
    try {
      // Server-filtered to `text_embedding` so the picker isn't crowded by
      // chat models (same rationale as the memory admin store).
      const body = await ApiClient.LlmModel.list({
        capability: 'text_embedding',
        page: 1,
        perPage: 200,
      })
      set(s => {
        s.embeddingModels = body.models.map(toRow)
        s.loadingModels = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load models'
        s.loadingModels = false
      })
    }
  }
