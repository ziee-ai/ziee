import { ApiClient } from '@/api-client'
import type { SummarizationAdminGet, SummarizationAdminSet } from '../state'

export default (set: SummarizationAdminSet, _get: SummarizationAdminGet) =>
  async () => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.SummarizationAdmin.get()
      set(s => {
        s.settings = row
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load summarization settings'
        s.loading = false
      })
    }
  }
