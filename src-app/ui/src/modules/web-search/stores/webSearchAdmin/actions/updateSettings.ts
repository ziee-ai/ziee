import { ApiClient } from '@/api-client'
import type {
  UpdateWebSearchSettingsRequest,
  WebSearchSettings,
} from '@/api-client/types'
import type { WebSearchAdminState } from '../state'

/** Lazy action — saves the global settings. Its own chunk. */
export default (set: (fn: (s: WebSearchAdminState) => void) => void) =>
  async (patch: UpdateWebSearchSettingsRequest): Promise<WebSearchSettings> => {
    set(s => {
      s.savingSettings = true
      s.error = null
    })
    try {
      const row = await ApiClient.WebSearch.updateSettings(patch)
      set(s => {
        s.settings = row
        s.savingSettings = false
      })
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.savingSettings = false
      })
      throw error
    }
  }
