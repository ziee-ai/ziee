import { ApiClient } from '@/api-client'
import type { HubInstalledResponse, HubInstalledRow } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

/**
 * Backs the "Installed" hub tab — every tracked hub install the caller can see
 * (per-user installs always; system-wide installs when the caller has
 * `hub::catalog::read`).
 */
export const HubInstalled = defineStore('HubInstalled', {
  immer: true,
  state: {
    items: [] as HubInstalledRow[],
    catalogVersion: null as string | null,
    loading: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    loadInstalled: async () => {
      if (get().loading) return
      set({ loading: true, error: null })
      try {
        const resp: HubInstalledResponse = await ApiClient.Hub.getInstalled()
        set({ items: resp.items, catalogVersion: resp.catalog_version, loading: false })
      } catch (error: any) {
        // Keep previously-loaded items on a refetch failure — a transient error
        // shouldn't blank a list the user was viewing.
        set({ error: error?.message || 'Failed to load installed hub items', loading: false })
      }
    },
  }),
  init: ({ actions }) => {
    void actions.loadInstalled()
  },
})

export const useHubInstalledStore = HubInstalled.store
