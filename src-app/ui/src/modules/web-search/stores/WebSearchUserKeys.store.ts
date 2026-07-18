import { ApiClient } from '@/api-client'
import { Permissions, type UserProviderKeyCatalogEntry } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * The calling user's OWN web-search provider keys. Each user may supply their
 * own key for a key-accepting provider (Brave, …); resolved before the shared
 * deployment key. Raw key never returned. Self-gated on `web_search::use`.
 */
export const WebSearchUserKeys = defineStore('WebSearchUserKeys', {
  immer: true,
  state: {
    providers: [] as UserProviderKeyCatalogEntry[],
    loading: false,
    savingProvider: null as string | null,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      if (!hasPermissionNow(Permissions.WebSearchUse)) return
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const res = await ApiClient.WebSearch.listUserKeys()
        set(s => {
          s.providers = res.providers
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.error =
            error instanceof Error ? error.message : 'Failed to load your web search keys'
          s.loading = false
        })
      }
    }
    return {
      load,
      saveKey: async (provider: string, apiKey: string) => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          const res = await ApiClient.WebSearch.saveUserKey({ provider, api_key: apiKey })
          set(s => {
            s.providers = res.providers
            s.savingProvider = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Failed to save key'
            s.savingProvider = null
          })
          throw error
        }
      },
      clearKey: async (provider: string) => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          await ApiClient.WebSearch.deleteUserKey({ provider })
          await load()
          set(s => {
            s.savingProvider = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Failed to clear key'
            s.savingProvider = null
          })
          throw error
        }
      },
    }
  },
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.WebSearchUse)) return
      void actions.load()
    }
    on('sync:web_search_user_key', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useWebSearchUserKeysStore = WebSearchUserKeys.store
