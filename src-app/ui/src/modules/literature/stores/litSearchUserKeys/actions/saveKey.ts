import { ApiClient } from '@/api-client'
import type { LitSearchUserKeysGet, LitSearchUserKeysSet } from '../state'

export default (set: LitSearchUserKeysSet, _get: LitSearchUserKeysGet) => {
  return async (connector: string, apiKey: string) => {
    set(s => {
      s.savingConnector = connector
      s.error = null
    })
    try {
      const res = await ApiClient.LitSearch.saveUserKey({ connector, api_key: apiKey })
      set(s => {
        s.connectors = res.connectors
        s.savingConnector = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to save key'
        s.savingConnector = null
      })
      throw error
    }
  }
}
