import { ApiClient } from '@/api-client'
import type { LitSearchUserKeysGet, LitSearchUserKeysSet } from '../state'
import loadFactory from './load'

export default (set: LitSearchUserKeysSet, get: LitSearchUserKeysGet) => {
  const load = loadFactory(set, get)
  return async (connector: string) => {
    set(s => {
      s.savingConnector = connector
      s.error = null
    })
    try {
      await ApiClient.LitSearch.deleteUserKey({ connector })
      await load()
      set(s => {
        s.savingConnector = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to clear key'
        s.savingConnector = null
      })
      throw error
    }
  }
}
