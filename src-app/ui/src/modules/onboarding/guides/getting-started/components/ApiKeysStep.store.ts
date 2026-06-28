import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { message } from 'antd'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { ProviderWithModels } from '@/api-client/types'

interface ApiKeysStepStore {
  providers: ProviderWithModels[]
  userKeys: Record<string, { masked_key: string }>
  enteredApiKeys: Record<string, string>
  loadingProviders: boolean
  providersError: string | null

  __init__: {
    providers?: () => void
  }

  setApiKey: (providerId: string, value: string) => void
  loadProviders: () => Promise<void>
  saveKey: (providerId: string, apiKey: string) => Promise<void>
  reset: () => void
}

export const useApiKeysStepStore = create<ApiKeysStepStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      providers: [],
      userKeys: {},
      enteredApiKeys: {},
      loadingProviders: false,
      providersError: null,

      __init__: {
        providers: () => { get().loadProviders() },
      },

      setApiKey: (providerId: string, value: string) => {
        set(draft => {
          draft.enteredApiKeys[providerId] = value
        })
      },

      loadProviders: async () => {
        set(state => {
          state.loadingProviders = true
          state.providersError = null
        })
        try {
          const [providersRes, keysRes] = await Promise.all([
            ApiClient.LlmProvider.getUserLlmProviders(undefined, undefined),
            ApiClient.LlmProvider.listUserApiKeys(undefined, undefined),
          ])
          set(state => {
            // Local providers authenticate via an internal proxy token, not a
            // user API key — exclude them from the key-entry list.
            state.providers = providersRes.providers.filter(
              p => p.enabled && p.provider_type !== 'local',
            )
            state.userKeys = Object.fromEntries(
              keysRes.keys.map(k => [k.provider_id, { masked_key: k.masked_key }]),
            )
            state.loadingProviders = false
          })
        } catch (error: any) {
          console.error('[ApiKeysStep] loadProviders error:', error)
          set(state => {
            state.providersError = error.message || 'Failed to load providers'
            state.loadingProviders = false
          })
        }
      },

      saveKey: async (providerId: string, apiKey: string) => {
        try {
          await ApiClient.LlmProvider.saveUserApiKey(
            { provider_id: providerId, api_key: apiKey },
            undefined,
          )
          await get().loadProviders()
          message.success('API key saved')
        } catch (error: any) {
          message.error(error.message || 'Failed to save API key')
          throw error
        }
      },

      reset: () => {
        set(draft => {
          draft.enteredApiKeys = {}
          // providers, userKeys, loadingProviders, providersError are intentionally
          // NOT reset — they are API cache and __init__.providers won't re-trigger
          // after reset, so clearing them causes an empty state on the next visit
        })
      },
    })),
  ),
)

export const ApiKeysStepStoreProxy = createStoreProxy(useApiKeysStepStore)
