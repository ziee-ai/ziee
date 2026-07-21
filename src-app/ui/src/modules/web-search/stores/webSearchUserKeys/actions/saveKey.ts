import { ApiClient } from '@/api-client'
import type { WebSearchUserKeysGet, WebSearchUserKeysSet } from '../state'

export default (set: WebSearchUserKeysSet, _get: WebSearchUserKeysGet) =>
  async (provider: string, apiKey: string): Promise<void> => {
    set(s => {
      s.savingProvider = provider
      s.error = null
    })
    try {
      const res = await ApiClient.WebSearch.saveUserKey({ provider, api_key: apiKey })
      set(s => {
        s.providers = res.providers
        s.savingProvider = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to save key'
        s.savingProvider = null
      })
      throw error
    }
  }
