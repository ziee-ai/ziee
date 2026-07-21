import { ApiClient } from '@/api-client'
import type { UserLlmProvidersGet, UserLlmProvidersSet } from '../state'
import loadFactory from './load'

export default (set: UserLlmProvidersSet, get: UserLlmProvidersGet) => {
  const load = loadFactory(set, get)
  return async (providerId: string, apiKey: string) => {
    set(state => {
      state.saving = true
    })
    try {
      await ApiClient.LlmProvider.saveUserApiKey(
        { provider_id: providerId, api_key: apiKey },
        undefined,
      )
      await load()
      // Success/error feedback is shown by the calling page (avoid double toast).
    } finally {
      set(state => {
        state.saving = false
      })
    }
  }
}
