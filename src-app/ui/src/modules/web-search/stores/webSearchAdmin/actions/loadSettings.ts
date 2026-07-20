import { ApiClient } from '@/api-client'
import type { WebSearchAdminState } from '../state'

/** Lazy action — loads the global web-search settings row. Its own chunk. */
export default (set: (fn: (s: WebSearchAdminState) => void) => void) =>
  async (): Promise<void> => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.WebSearch.getSettings()
      set(s => {
        s.settings = row
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load web search settings'
        s.loading = false
      })
    }
  }
