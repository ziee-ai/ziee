import { ApiClient } from '@/api-client'
import type { FileRagAdminSet } from '../state'
import toRow from './_toRow'

export default (set: FileRagAdminSet) =>
  async () => {
    try {
      const body = await ApiClient.LlmModel.list({
        capability: 'rerank',
        page: 1,
        perPage: 200,
      })
      set(s => {
        s.rerankerModels = body.models.map(toRow)
      })
    } catch {
      /* non-fatal — the reranker section shows the hub nudge when empty */
    }
  }
