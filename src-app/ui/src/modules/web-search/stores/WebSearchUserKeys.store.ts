import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type UserProviderKeyCatalogEntry,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * The calling user's OWN web-search provider keys. Each user may supply their
 * own key for a key-accepting provider (Brave, …); it is resolved before the
 * shared deployment key. The raw key is never returned — only masked state +
 * whether a shared deployment key exists as a fallback.
 *
 * Gated on `web_search::use` (held by the Users group), so the store self-gates
 * every load/refetch on that permission (no 403 on reconnect).
 */
interface WebSearchUserKeysStore {
  providers: UserProviderKeyCatalogEntry[]
  loading: boolean
  /** The provider whose key is being saved/cleared, or null. */
  savingProvider: string | null
  error: string | null

  __init__: {
    __store__?: () => void
    providers: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  saveKey: (provider: string, apiKey: string) => Promise<void>
  clearKey: (provider: string) => Promise<void>
}

const load = async (set: (fn: (s: WebSearchUserKeysStore) => void) => void) => {
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

export const useWebSearchUserKeysStore = create<WebSearchUserKeysStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      providers: [],
      loading: false,
      savingProvider: null,
      error: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'WebSearchUserKeys'
          const reload = () => {
            if (!hasPermissionNow(Permissions.WebSearchUse)) return
            void load(set)
          }
          eventBus.on('sync:web_search_user_key', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        providers: () => load(set),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('WebSearchUserKeys')
      },

      load: () => load(set),

      saveKey: async (provider, apiKey): Promise<void> => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          const res = await ApiClient.WebSearch.saveUserKey({
            provider,
            api_key: apiKey,
          })
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

      clearKey: async (provider): Promise<void> => {
        set(s => {
          s.savingProvider = provider
          s.error = null
        })
        try {
          await ApiClient.WebSearch.deleteUserKey({ provider })
          await load(set)
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
    })),
  ),
)
