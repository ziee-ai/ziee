import { ApiClient } from '@/api-client'
import type { LitSearchAdminGet, LitSearchAdminSet } from '../state'

export default (set: LitSearchAdminSet, _get: LitSearchAdminGet) =>
  async () => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.LitSearch.getSettings()
      set(s => {
        s.settings = row
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load literature search settings'
        s.loading = false
      })
    }
  }
