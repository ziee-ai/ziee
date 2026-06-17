import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type ProviderCatalogEntry,
  type UpdateProviderRequest,
  type UpdateWebSearchSettingsRequest,
  type WebSearchSettings,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface WebSearchAdminStore {
  settings: WebSearchSettings | null
  providers: ProviderCatalogEntry[]
  loading: boolean
  /** Global settings (enable / chain / caps) save in flight. */
  savingSettings: boolean
  /** Provider key being saved (its registry key), or null. Scopes the spinner
   *  to the one in-flight provider form instead of all of them. */
  savingProvider: string | null
  error: string | null

  __init__: {
    __store__?: () => void
    settings: () => Promise<void>
    providers: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  updateSettings: (
    patch: UpdateWebSearchSettingsRequest,
  ) => Promise<WebSearchSettings>
  updateProvider: (
    provider: string,
    body: UpdateProviderRequest,
  ) => Promise<void>
}

const loadSettings = async (
  set: (fn: (s: WebSearchAdminStore) => void) => void,
) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.WebSearch.getSettings()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load web search settings'
      s.loading = false
    })
  }
}

const loadProviders = async (
  set: (fn: (s: WebSearchAdminStore) => void) => void,
) => {
  try {
    const res = await ApiClient.WebSearch.getProviders()
    set(s => {
      s.providers = res.providers
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load search providers'
    })
  }
}

export const useWebSearchAdminStore = create<WebSearchAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      providers: [],
      loading: false,
      savingSettings: false,
      savingProvider: null,
      error: null,

      // Property-init loaders hit `web_search::admin::read`-gated endpoints.
      // Self-gate so non-admins never generate 403s.
      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'WebSearchAdmin'
          const reload = () => {
            if (!hasPermissionNow(Permissions.WebSearchAdminRead)) return
            void loadSettings(set)
            void loadProviders(set)
          }
          eventBus.on('sync:web_search_settings', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        settings: () =>
          hasPermissionNow(Permissions.WebSearchAdminRead)
            ? loadSettings(set)
            : Promise.resolve(),
        providers: () =>
          hasPermissionNow(Permissions.WebSearchAdminRead)
            ? loadProviders(set)
            : Promise.resolve(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('WebSearchAdmin')
      },

      load: async () => {
        await Promise.all([loadSettings(set), loadProviders(set)])
      },

      updateSettings: async (patch): Promise<WebSearchSettings> => {
        set(s => {
          s.savingSettings = true
          s.error = null
        })
        try {
          const row = await ApiClient.WebSearch.updateSettings(patch)
          set(s => {
            s.settings = row
            s.savingSettings = false
          })
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.savingSettings = false
          })
          throw error
        }
      },

      updateProvider: async (provider, body): Promise<void> => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          const res = await ApiClient.WebSearch.updateProvider({
            provider,
            ...body,
          })
          set(s => {
            s.providers = res.providers
            s.savingProvider = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.savingProvider = null
          })
          throw error
        }
      },
    })),
  ),
)
