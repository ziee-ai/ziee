import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type UserConnectorKeyCatalogEntry,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * The calling user's OWN lit-search connector keys. Each user may supply their
 * own key for a key-accepting connector (CORE, Semantic Scholar, …); it is
 * resolved before the shared deployment key. The raw key is never returned —
 * only masked state + whether a shared deployment key exists as a fallback.
 *
 * Gated on `lit_search::use` (held by the Users group), so the store self-gates
 * every load/refetch on that permission (no 403 on reconnect).
 */
interface LitSearchUserKeysStore {
  connectors: UserConnectorKeyCatalogEntry[]
  loading: boolean
  /** The connector whose key is being saved/cleared, or null. */
  savingConnector: string | null
  error: string | null

  __init__: {
    __store__?: () => void
    connectors: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  saveKey: (connector: string, apiKey: string) => Promise<void>
  clearKey: (connector: string) => Promise<void>
}

const load = async (set: (fn: (s: LitSearchUserKeysStore) => void) => void) => {
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

export const useLitSearchUserKeysStore = create<LitSearchUserKeysStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      connectors: [],
      loading: false,
      savingConnector: null,
      error: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'LitSearchUserKeys'
          const reload = () => {
            if (!hasPermissionNow(Permissions.LitSearchUse)) return
            void load(set)
          }
          eventBus.on('sync:lit_search_user_key', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        connectors: () => load(set),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LitSearchUserKeys')
      },

      load: () => load(set),

      saveKey: async (connector, apiKey): Promise<void> => {
        set(s => {
          s.savingConnector = connector
          s.error = null
        })
        try {
          const res = await ApiClient.LitSearch.saveUserKey({
            connector,
            api_key: apiKey,
          })
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

      clearKey: async (connector): Promise<void> => {
        set(s => {
          s.savingConnector = connector
          s.error = null
        })
        try {
          await ApiClient.LitSearch.deleteUserKey({ connector })
          await load(set)
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
    })),
  ),
)
