import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { ProviderWithModels } from '@/api-client/types'

interface UserLlmProvidersStore {
  providers: ProviderWithModels[]
  userKeys: Record<string, { masked_key: string }>
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    providers?: () => void
  }
  __destroy__?: () => void

  load: () => Promise<void>
  saveKey: (providerId: string, apiKey: string) => Promise<void>
  deleteKey: (providerId: string) => Promise<void>
}

export const useUserLlmProvidersStore = create<UserLlmProvidersStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      providers: [],
      userKeys: {},
      loading: false,
      saving: false,
      error: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'UserLlmProvidersStore'

          eventBus.on('llm_provider.created', async () => { await get().load() }, GROUP)
          eventBus.on('llm_provider.updated', async () => { await get().load() }, GROUP)
          eventBus.on('llm_provider.deleted', async () => { await get().load() }, GROUP)
        },
        providers: () => { get().load() },
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('UserLlmProvidersStore')
      },

      load: async () => {
        set(state => {
          state.loading = true
          state.error = null
        })
        try {
          const [providersRes, keysRes] = await Promise.all([
            ApiClient.LlmProvider.getUserLlmProviders(undefined, undefined),
            ApiClient.LlmProvider.listUserApiKeys(undefined, undefined),
          ])
          set(state => {
            state.providers = providersRes.providers.filter(p => p.enabled)
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
      },

      saveKey: async (providerId: string, apiKey: string) => {
        set(state => { state.saving = true })
        try {
          await ApiClient.LlmProvider.saveUserApiKey(
            { provider_id: providerId, api_key: apiKey },
            undefined,
          )
          await get().load()
        } finally {
          set(state => { state.saving = false })
        }
      },

      deleteKey: async (providerId: string) => {
        set(state => { state.saving = true })
        try {
          await ApiClient.LlmProvider.deleteUserApiKey(
            { provider_id: providerId },
            undefined,
          )
          await get().load()
        } finally {
          set(state => { state.saving = false })
        }
      },
    })),
  ),
)

export const UserLlmProvidersStoreProxy = createStoreProxy(useUserLlmProvidersStore)
