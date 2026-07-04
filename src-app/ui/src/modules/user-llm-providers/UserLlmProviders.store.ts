import { ApiClient } from '@/api-client'
import { Permissions, type ProviderWithModels } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { createStoreProxy } from '@/core/stores'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

export const UserLlmProviders = defineStore('UserLlmProviders', {
  immer: true,
  state: {
    providers: [] as ProviderWithModels[],
    userKeys: {} as Record<string, { masked_key: string }>,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      // Permission-gate the shell-eager-load fetch: the chat model selector
      // accesses this store on every chat render. Without
      // user_llm_providers::read the parallel GETs 403 for restricted users.
      if (!hasPermissionNow(Permissions.UserLlmProvidersRead)) return
      set(state => {
        state.loading = true
        state.error = null
      })
      try {
        const [providersRes, keysRes] = await Promise.all([
          ApiClient.LlmProvider.getUserLlmProviders({}, undefined),
          ApiClient.LlmProvider.listUserApiKeys(undefined, undefined),
        ])
        set(state => {
          // Local providers authenticate via an internal proxy token, not a
          // user API key — exclude them from the personal-key list.
          state.providers = sortProviders(
            providersRes.providers.filter(p => p.enabled && p.provider_type !== 'local'),
          )
          state.userKeys = Object.fromEntries(
            keysRes.keys.map(k => [k.provider_id, { masked_key: k.masked_key }]),
          )
          state.loading = false
        })
      } catch (error: any) {
        console.error('[UserLlmProviders] load error:', error)
        set(state => {
          state.error = error.message || 'Failed to load providers'
          state.loading = false
        })
      }
    }
    return {
      load,
      saveKey: async (providerId: string, apiKey: string) => {
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
      },
      deleteKey: async (providerId: string) => {
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
      },
    }
  },
  init: ({ on, actions }) => {
    on('llm_provider.created', () => void actions.load())
    on('llm_provider.updated', () => void actions.load())
    on('llm_provider.deleted', () => void actions.load())
    // Remote sync: an API key / provider / model changed on another device, or
    // we (re)connected. load() self-gates on UserLlmProvidersRead.
    const reload = () => void actions.load()
    on('sync:api_key', reload)
    on('sync:user_llm_provider', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useUserLlmProvidersStore = UserLlmProviders.store
export const UserLlmProvidersStoreProxy = createStoreProxy(useUserLlmProvidersStore)
