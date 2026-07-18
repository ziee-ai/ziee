import { Permissions, type LlmProvider } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { defineLocalStore } from '@ziee/framework/store-kit'

/**
 * PRIVATE, per-widget store (one instance per group row) — MIGRATED from a global
 * `groupId`-keyed singleton to `defineLocalStore`.
 *
 * Before: one global store held a `Map<groupId, providers>` + an `allProviders`
 * cache, was registered in `Stores.LlmProviderGroupWidget`, and every widget
 * shared it — so the widget needed a mount `useEffect` to fetch, and the store
 * carried Map bookkeeping + a manual `__destroy__`.
 *
 * After: each mounted widget owns just ITS group's providers. `init` fetches on
 * MOUNT (so it's populated after a reload with no consumer-side effect) and its
 * event listeners auto-unsubscribe on UNMOUNT. No global Map, no
 * `Stores.LlmProviderGroupWidget`, no GROUP string, no `__destroy__`.
 */
export const LlmProviderGroupWidgetStore = defineLocalStore({
  immer: true,
  state: {
    groupId: '' as string,
    providers: [] as LlmProvider[],
    loading: false,
    error: null as string | null,
  },

  actions: (set, get) => {
    const load = async (force = false) => {
      const groupId = get().groupId
      if (!groupId) return
      if (get().loading && !force) return
      set(d => {
        d.loading = true
        d.error = null
      })
      try {
        const response = await ApiClient.Group.getProviders({ group_id: groupId })
        set(d => {
          // Defensive: never assign a non-array into `providers` — the widget
          // reads `providers.length` unconditionally, so a malformed/empty
          // response ({} or missing field) would crash the whole group row.
          d.providers = Array.isArray(response.providers) ? response.providers : []
          d.loading = false
        })
      } catch (error) {
        console.error(`Failed to load providers for group ${groupId}:`, error)
        set(d => {
          d.loading = false
          d.error =
            error instanceof Error ? error.message : 'Failed to load providers'
        })
      }
    }

    return {
      load,
      /** Re-point this instance at a different group (defensive — parents should
       *  key widgets by group.id, but group.id can change in place). */
      setGroup: (groupId: string) => {
        if (get().groupId === groupId) return
        set(d => {
          d.groupId = groupId
        })
        void load(true)
      },
    }
  },

  // Runs on MOUNT; every `on(...)` auto-unsubscribes on UNMOUNT.
  init: ({ on, get, set, actions }) => {
    // `GET /api/groups/{id}/providers` requires llm_providers::read (not
    // user-held); guard the eager mount fetch so a groups-admin without it
    // (viewing the user-groups page) doesn't 403.
    if (hasPermissionNow(Permissions.LlmProvidersRead)) {
      void actions.load()
    }

    // Real-time updates, scoped to THIS instance's group.
    on('llm_provider.group_providers_changed', async event => {
      if (event.data.groupId !== get().groupId) return
      await actions.load(true)
    })
    on('llm_provider.created', () => {
      void actions.load(true)
    })
    on('llm_provider.updated', event => {
      set(d => {
        const i = d.providers.findIndex(p => p.id === event.data.provider.id)
        if (i !== -1) d.providers[i] = event.data.provider
      })
    })
    on('llm_provider.deleted', event => {
      set(d => {
        d.providers = d.providers.filter(p => p.id !== event.data.providerId)
      })
    })
  },
})
