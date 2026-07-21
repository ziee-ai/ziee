import { ApiClient } from '@/api-client'
import type { UserLlmProvidersGet, UserLlmProvidersSet } from '../state'
import loadFactory from './load'

export default (set: UserLlmProvidersSet, get: UserLlmProvidersGet) => {
  const load = loadFactory(set, get)
  return async (providerId: string) => {
    set(state => {
      state.saving = true
    })
    try {
      await ApiClient.LlmProvider.deleteUserApiKey({ provider_id: providerId }, undefined)
      await load()
    } finally {
      set(state => {
        state.saving = false
      })
    }
  }
}
