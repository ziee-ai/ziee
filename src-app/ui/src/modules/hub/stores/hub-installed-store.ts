import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubInstalledResponse, HubInstalledRow } from '@/api-client/types'

/**
 * Backs the "Installed" hub tab — every tracked hub install the
 * caller can see (per-user installs always; system-wide installs
 * when the caller has `hub::catalog::read`). Replaces the prior
 * Updates store, which only surfaced rows behind the catalog.
 */
interface HubInstalledState {
  items: HubInstalledRow[]
  catalogVersion: string | null
  loading: boolean
  error: string | null

  loadInstalled: () => Promise<void>

  __init__: {
    items: () => Promise<void>
  }
}

export const useHubInstalledStore = create<HubInstalledState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubInstalledState => ({
        items: [],
        catalogVersion: null,
        loading: false,
        error: null,

        loadInstalled: async () => {
          if (get().loading) return
          set({ loading: true, error: null })
          try {
            const resp: HubInstalledResponse =
              await ApiClient.Hub.getInstalled()
            set({
              items: resp.items,
              catalogVersion: resp.catalog_version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error?.message || 'Failed to load installed hub items',
              loading: false,
              items: [],
            })
          }
        },

        __init__: {
          items: () => get().loadInstalled(),
        },
      }),
    ),
  ),
)
