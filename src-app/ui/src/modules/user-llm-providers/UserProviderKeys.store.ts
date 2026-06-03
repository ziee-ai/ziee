import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'

/**
 * Per-provider API-key cache for user-supplied LLM provider keys.
 * Lives at Stores.UserProviderKeys (registered in
 * modules/user-llm-providers/module.tsx). Type augmentation lives in
 * the module's types.ts (the codebase convention used by the
 * other stores in this module).
 *
 * Used by the chat-extension's ProviderApiKeyModal to save a user's
 * API key for a provider that the chat model picker surfaces as
 * unconfigured.
 */
interface UserProviderKeysState {
  keys: Record<string, { masked_key: string }>
  saving: boolean
  __init__: boolean

  // Actions
  loadKeys: () => Promise<void>
  saveKey: (providerId: string, apiKey: string) => Promise<void>
}

export const useUserProviderKeysStore = create<UserProviderKeysState>()(
  subscribeWithSelector((set, get) => ({
    keys: {},
    saving: false,
    __init__: false,

    loadKeys: async () => {
      if (get().__init__) return
      const response = await ApiClient.LlmProvider.listUserApiKeys(
        undefined,
        undefined,
      )
      const keysMap: Record<string, { masked_key: string }> = {}
      for (const entry of response.keys) {
        keysMap[entry.provider_id] = { masked_key: entry.masked_key }
      }
      set({ keys: keysMap, __init__: true })
    },

    saveKey: async (providerId: string, apiKey: string) => {
      set({ saving: true })
      try {
        await ApiClient.LlmProvider.saveUserApiKey(
          { provider_id: providerId, api_key: apiKey },
          undefined,
        )
        // Refresh keys after save
        set({ __init__: false })
        await get().loadKeys()
      } finally {
        set({ saving: false })
      }
    },

  })),
)
