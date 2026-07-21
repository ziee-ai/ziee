import { ApiClient } from '@/api-client'
import type { WebSearchUserKeysGet, WebSearchUserKeysSet } from '../state'
import loadFactory from './load'

export default (set: WebSearchUserKeysSet, get: WebSearchUserKeysGet) =>
  async (provider: string): Promise<void> => {
    const load = loadFactory(set, get)
    set(s => {
      s.savingProvider = provider
      s.error = null
    })
    try {
      await ApiClient.WebSearch.deleteUserKey({ provider })
      await load()
      set(s => {
        s.savingProvider = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to clear key'
        s.savingProvider = null
      })
      throw error
    }
  }
