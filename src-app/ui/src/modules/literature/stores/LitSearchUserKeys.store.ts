import { ApiClient } from '@/api-client'
import { type UserConnectorKeyCatalogEntry } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * The calling user's OWN lit-search connector keys. Each user may supply their
 * own key for a key-accepting connector (CORE, Semantic Scholar, …); it is
 * resolved before the shared deployment key. Raw key never returned — only
 * masked state + whether a shared deployment key exists as a fallback.
 * Self-gated on `lit_search::use` (no 403 on reconnect).
 */
export const LitSearchUserKeys = defineStore('LitSearchUserKeys', {
  immer: true,
  state: {
    connectors: [] as UserConnectorKeyCatalogEntry[],
    loading: false,
    savingConnector: null as string | null,
    error: null as string | null,
  },
  actions: set => {
    const load = async () => {
      if (!hasPermissionNow(Permissions.LitSearchUse)) return
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const res = await ApiClient.LitSearch.listUserKeys()
        set(s => {
          s.connectors = res.connectors
          s.loading = false
        })
      } catch (error) {
        set(s => {
          s.error =
            error instanceof Error ? error.message : 'Failed to load your literature keys'
          s.loading = false
        })
      }
    }
    return {
      load,
      saveKey: async (connector: string, apiKey: string) => {
        set(s => {
          s.savingConnector = connector
          s.error = null
        })
        try {
          const res = await ApiClient.LitSearch.saveUserKey({ connector, api_key: apiKey })
          set(s => {
            s.connectors = res.connectors
            s.savingConnector = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Failed to save key'
            s.savingConnector = null
          })
          throw error
        }
      },
      clearKey: async (connector: string) => {
        set(s => {
          s.savingConnector = connector
          s.error = null
        })
        try {
          await ApiClient.LitSearch.deleteUserKey({ connector })
          await load()
          set(s => {
            s.savingConnector = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Failed to clear key'
            s.savingConnector = null
          })
          throw error
        }
      },
    }
  },
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.LitSearchUse)) return
      void actions.load()
    }
    on('sync:lit_search_user_key', reload)
    on('sync:reconnect', reload)
    void actions.load()
  },
})

export const useLitSearchUserKeysStore = LitSearchUserKeys.store
