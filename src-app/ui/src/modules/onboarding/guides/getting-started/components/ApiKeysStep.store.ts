import { ApiClient } from '@/api-client'
import type { ProviderWithModels } from '@/api-client/types'
import { message } from '@ziee/kit'
import { defineStore } from '@ziee/framework/store-kit'
import { createStoreProxy } from '@ziee/framework/stores'

export const ApiKeysStep = defineStore('ApiKeysStep', {
  immer: true,
  state: {
    providers: [] as ProviderWithModels[],
    userKeys: {} as Record<string, { masked_key: string }>,
    enteredApiKeys: {} as Record<string, string>,
    loadingProviders: false,
    providersError: null as string | null,
  },
  actions: set => {
    const loadProviders = async () => {
      set(state => {
        state.loadingProviders = true
        state.providersError = null
      })
      try {
        const [providersRes, keysRes] = await Promise.all([
          ApiClient.LlmProvider.getUserLlmProviders({}, undefined),
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
    }
    return {
      loadProviders,
      setApiKey: (providerId: string, value: string) => {
        set(draft => {
          draft.enteredApiKeys[providerId] = value
        })
      },
      saveKey: async (providerId: string, apiKey: string) => {
        try {
          await ApiClient.LlmProvider.saveUserApiKey(
            { provider_id: providerId, api_key: apiKey },
            undefined,
          )
          await loadProviders()
          message.success('API key saved')
        } catch (error: any) {
          message.error(error.message || 'Failed to save API key')
          throw error
        }
      },
      reset: () => {
        set(draft => {
          draft.enteredApiKeys = {}
          // providers/userKeys/loading/error intentionally NOT reset — API cache;
          // init won't re-trigger after reset, so clearing them would blank the
          // next visit.
        })
      },
    }
  },
  init: ({ actions }) => {
    void actions.loadProviders()
  },
})

export const useApiKeysStepStore = ApiKeysStep.store
export const ApiKeysStepStoreProxy = createStoreProxy(useApiKeysStepStore)
