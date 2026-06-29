import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Permissions, type ProviderWithModels } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { createStoreProxy, Stores } from '@/core/stores'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

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

          eventBus.on(
            'llm_provider.created',
            async () => {
              await get().load()
            },
            GROUP,
          )
          eventBus.on(
            'llm_provider.updated',
            async () => {
              await get().load()
            },
            GROUP,
          )
          eventBus.on(
            'llm_provider.deleted',
            async () => {
              await get().load()
            },
            GROUP,
          )

          // Remote sync: an API key or provider/model changed on another
          // device, or we (re)connected. `load()` self-gates on
          // UserLlmProvidersRead and refetches its own scoped view.
          const reload = () => void get().load()
          eventBus.on('sync:api_key', reload, GROUP)
          eventBus.on('sync:user_llm_provider', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        providers: () => {
          get().load()
        },
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('UserLlmProvidersStore')
      },

      load: async () => {
        // Permission-gate the shell-eager-load fetch (audit
        // follow-up): the chat model selector accesses this store
        // on every chat render. Without user_llm_providers::read
        // the parallel GETs (/api/user-llm-providers + /api-keys)
        // 403 for restricted users.
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
              providersRes.providers.filter(
                p => p.enabled && p.provider_type !== 'local',
              ),
            )
            state.userKeys = Object.fromEntries(
              keysRes.keys.map(k => [
                k.provider_id,
                { masked_key: k.masked_key },
              ]),
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
        set(state => {
          state.saving = true
        })
        try {
          await ApiClient.LlmProvider.saveUserApiKey(
            { provider_id: providerId, api_key: apiKey },
            undefined,
          )
          await get().load()
          // Success/error feedback is shown by the calling page via the
          // contextual `App.useApp()` message — keep UI toasts out of the store
          // (showing both here and at the page produced a double toast on save).
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
          await ApiClient.LlmProvider.deleteUserApiKey(
            { provider_id: providerId },
            undefined,
          )
          await get().load()
        } finally {
          set(state => {
            state.saving = false
          })
        }
      },
    })),
  ),
)

export const UserLlmProvidersStoreProxy = createStoreProxy(
  useUserLlmProvidersStore,
)
