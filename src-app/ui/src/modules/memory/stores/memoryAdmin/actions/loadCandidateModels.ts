import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

const toRow = (m: import('@/api-client/types').LlmModel) => ({
  id: m.id,
  name: m.name,
  display_name: m.display_name,
  provider_id: m.provider_id,
  capabilities: m.capabilities,
})

export default (set: MemoryAdminSet, _get: MemoryAdminGet) => async () => {
  set(s => {
    s.loadingModels = true
  })
  try {
    // Two capped fetches: embedding picker (server-filtered) + all models
    // (extraction picker keeps the non-embedders).
    const [allBody, embeddingBody] = await Promise.all([
      ApiClient.LlmModel.list({ page: 1, perPage: 200 }),
      ApiClient.LlmModel.list({ capability: 'text_embedding', page: 1, perPage: 200 }),
    ])
    set(s => {
      // Extraction picker = all models MINUS embedders ("not an embedder"
      // rather than "is chat", so a chat model with no capability flag
      // still appears).
      s.availableModels = allBody.models.map(toRow).filter(m => !m.capabilities?.text_embedding)
      s.embeddingModels = embeddingBody.models.map(toRow)
      s.loadingModels = false
    })
  } catch (error) {
    set(s => {
      s.error = error instanceof Error ? error.message : 'Failed to load models'
      s.loadingModels = false
    })
  }
}
