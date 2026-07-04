/**
 * Server self-update notification store (web UI, admin-only). Wraps
 * ApiClient.ServerUpdate.getStatus() — notification only, no download/install.
 */
import { ApiClient } from '@/api-client'
import { type StoreProxy } from '@/core/stores'
import { defineStore } from '@/core/store-kit'

const DISMISSED_VERSION_KEY = 'ziee:server-update:dismissed-version'

interface ServerUpdateState {
  currentVersion: string | null
  latestVersion: string | null
  updateAvailable: boolean
  releaseUrl: string | null
  notes: string | null
  enabled: boolean
  checkedAt: string | null
  dismissed: boolean
  loading: boolean
  error: string | null
  loadStatus: () => Promise<void>
  dismiss: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ServerUpdate: StoreProxy<ServerUpdateState>
  }
}

export const ServerUpdate = defineStore('ServerUpdate', {
  immer: true,
  state: {
    currentVersion: null as string | null,
    latestVersion: null as string | null,
    updateAvailable: false,
    releaseUrl: null as string | null,
    notes: null as string | null,
    enabled: true,
    checkedAt: null as string | null,
    dismissed: false,
    loading: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    loadStatus: async () => {
      set(st => {
        st.loading = true
        st.error = null
      })
      try {
        const s = await ApiClient.ServerUpdate.getStatus(undefined, undefined)
        set(st => {
          st.currentVersion = s.current_version
          st.latestVersion = s.latest_version ?? null
          st.updateAvailable = s.update_available
          st.releaseUrl = s.release_url ?? null
          st.notes = s.notes ?? null
          st.enabled = s.enabled
          st.checkedAt = s.checked_at ?? null
          const dismissedVersion = localStorage.getItem(DISMISSED_VERSION_KEY)
          st.dismissed =
            s.update_available && !!dismissedVersion && dismissedVersion === (s.latest_version ?? null)
          st.loading = false
        })
      } catch (e) {
        set(st => {
          st.loading = false
          st.error = e instanceof Error ? e.message : 'Failed to load update status'
        })
      }
    },
    dismiss: () => {
      const v = get().latestVersion
      if (v) localStorage.setItem(DISMISSED_VERSION_KEY, v)
      set(st => {
        st.dismissed = true
      })
    },
  }),
  // Was `__init__.updateAvailable` — hydrate on first access.
  init: ({ actions }) => {
    void actions.loadStatus()
  },
})

export const useServerUpdateStore = ServerUpdate.store
