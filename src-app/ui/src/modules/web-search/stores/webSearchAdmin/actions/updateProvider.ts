import { ApiClient } from '@/api-client'
import type { UpdateProviderRequest } from '@/api-client/types'
import type { WebSearchAdminState } from '../state'

/** Lazy action — saves one provider's config. Its own chunk. */
export default (set: (fn: (s: WebSearchAdminState) => void) => void) =>
  async (provider: string, body: UpdateProviderRequest): Promise<void> => {
    set(s => {
      s.savingProvider = provider
      s.error = null
    })
    try {
      const res = await ApiClient.WebSearch.updateProvider({ provider, ...body })
      set(s => {
        s.providers = res.providers
        s.savingProvider = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.savingProvider = null
      })
      throw error
    }
  }
