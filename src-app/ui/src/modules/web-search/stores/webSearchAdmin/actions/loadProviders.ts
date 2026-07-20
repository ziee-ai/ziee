import { ApiClient } from '@/api-client'
import type { WebSearchAdminState } from '../state'

/** Lazy action — loads the provider catalog. Its own chunk. */
export default (set: (fn: (s: WebSearchAdminState) => void) => void) =>
  async (): Promise<void> => {
    try {
      const res = await ApiClient.WebSearch.getProviders()
      set(s => {
        s.providers = res.providers
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load search providers'
      })
    }
  }
