import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubUpdatesResponse, HubUpdateRow } from '@/api-client/types'

/**
 * Admin-only: installed hub_entities whose hub_version lags the
 * current catalog version. Backs the Updates tab.
 */
interface HubUpdatesState {
  updates: HubUpdateRow[]
  catalogVersion: string | null
  loading: boolean
  error: string | null

  loadUpdates: () => Promise<void>

  __init__: {
    updates: () => Promise<void>
  }
}

export const useHubUpdatesStore = create<HubUpdatesState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubUpdatesState => ({
        updates: [],
        catalogVersion: null,
        loading: false,
        error: null,

        loadUpdates: async () => {
          if (get().loading) return
          set({ loading: true, error: null })
          try {
            const resp: HubUpdatesResponse = await ApiClient.Hub.getUpdates()
            set({
              updates: resp.updates,
              catalogVersion: resp.catalog_version,
              loading: false,
            })
          } catch (error: any) {
            // 403 expected for non-admin — leave empty + log.
            set({
              error: error?.message || 'Failed to load hub updates',
              loading: false,
              updates: [],
            })
          }
        },

        __init__: {
          updates: () => get().loadUpdates(),
        },
      }),
    ),
  ),
)
