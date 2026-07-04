import { ApiClient } from '@/api-client'
import { defineStore } from '@/core/store-kit'

/**
 * Per-provider API-key cache for user-supplied LLM provider keys
 * (Stores.UserProviderKeys). Used by the chat ProviderApiKeyModal.
 */
export const UserProviderKeys = defineStore('UserProviderKeys', {
  state: {
    keys: {} as Record<string, { masked_key: string }>,
    saving: false,
    // Renamed from `__init__` (a reserved store-kit lifecycle key).
    initialized: false,
  },
  actions: (set, get) => {
    const loadKeys = async () => {
      if (get().initialized) return
      const response = await ApiClient.LlmProvider.listUserApiKeys(undefined, undefined)
      const keysMap: Record<string, { masked_key: string }> = {}
      for (const entry of response.keys) {
        keysMap[entry.provider_id] = { masked_key: entry.masked_key }
      }
      set({ keys: keysMap, initialized: true })
    }
    return {
      loadKeys,
      saveKey: async (providerId: string, apiKey: string) => {
        set({ saving: true })
        try {
          await ApiClient.LlmProvider.saveUserApiKey(
            { provider_id: providerId, api_key: apiKey },
            undefined,
          )
          set({ initialized: false }) // refresh after save
          await loadKeys()
        } finally {
          set({ saving: false })
        }
      },
    }
  },
})

export const useUserProviderKeysStore = UserProviderKeys.store
