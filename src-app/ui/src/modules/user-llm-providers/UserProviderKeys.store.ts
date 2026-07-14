import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

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
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `profile::read` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.ProfileRead)) return
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
  init: ({ on, set, actions }) => {
    // Cross-device sync: a key saved/removed on another device (or a missed
    // event across a dropped stream) invalidates this per-provider cache. Reset
    // `initialized` so the guarded loadKeys() actually refetches; loadKeys() is
    // permission-gated internally (profile::read).
    const reload = () => {
      set({ initialized: false })
      void actions.loadKeys()
    }
    on('sync:api_key', reload)
    on('sync:reconnect', reload)
  },
})

export const useUserProviderKeysStore = UserProviderKeys.store
