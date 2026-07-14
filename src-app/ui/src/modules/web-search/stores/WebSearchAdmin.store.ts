import { ApiClient } from '@/api-client'
import {
  Permissions,
  type ProviderCatalogEntry,
  type UpdateProviderRequest,
  type UpdateWebSearchSettingsRequest,
  type WebSearchSettings,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

export const WebSearchAdmin = defineStore('WebSearchAdmin', {
  immer: true,
  state: {
    settings: null as WebSearchSettings | null,
    providers: [] as ProviderCatalogEntry[],
    loading: false,
    /** Global settings (enable / chain / caps) save in flight. */
    savingSettings: false,
    /** Provider key being saved (its registry key), or null — scopes the spinner. */
    savingProvider: null as string | null,
    error: null as string | null,
  },
  actions: set => {
    const loadSettings = async () => {
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
    const loadProviders = async () => {
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
    return {
      loadSettings,
      loadProviders,
      load: async () => {
        await Promise.all([loadSettings(), loadProviders()])
      },
      updateSettings: async (
        patch: UpdateWebSearchSettingsRequest,
      ): Promise<WebSearchSettings> => {
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
      updateProvider: async (
        provider: string,
        body: UpdateProviderRequest,
      ): Promise<void> => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          const res = await ApiClient.WebSearch.updateProvider({ provider, ...body })
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
    }
  },
  // Loaders hit `web_search::admin::read`-gated endpoints. Self-gate so
  // non-admins never generate 403s (incl. on the audience-agnostic reconnect).
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.WebSearchAdminRead)) return
      void actions.loadSettings()
      void actions.loadProviders()
    }
    on('sync:web_search_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})

export const useWebSearchAdminStore = WebSearchAdmin.store
