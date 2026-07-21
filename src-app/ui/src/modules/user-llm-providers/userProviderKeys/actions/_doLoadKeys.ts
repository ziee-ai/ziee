import { ApiClient } from '@/api-client'
import type { UserProviderKeysGet, UserProviderKeysSet } from '../state'

export default (set: UserProviderKeysSet, _get: UserProviderKeysGet) =>
  async () => {
    const response = await ApiClient.LlmProvider.listUserApiKeys(undefined, undefined)
    const keysMap: Record<string, { masked_key: string }> = {}
    for (const entry of response.keys) {
      keysMap[entry.provider_id] = { masked_key: entry.masked_key }
    }
    set({ keys: keysMap })
  }
