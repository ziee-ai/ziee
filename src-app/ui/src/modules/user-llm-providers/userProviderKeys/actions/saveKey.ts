import { ApiClient } from '@/api-client'
import type { UserProviderKeysGet, UserProviderKeysSet } from '../state'
import doLoadKeysFactory from './_doLoadKeys'

export default (set: UserProviderKeysSet, _get: UserProviderKeysGet) => {
  const doLoadKeys = doLoadKeysFactory(set, _get)
  return async (providerId: string, apiKey: string) => {
    set({ saving: true })
    try {
      await ApiClient.LlmProvider.saveUserApiKey(
        { provider_id: providerId, api_key: apiKey },
        undefined,
      )
      set({ initialized: false }) // refresh after save
      await doLoadKeys()
      set({ initialized: true })
    } finally {
      set({ saving: false })
    }
  }
}
