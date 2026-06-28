/**
 * Server self-update notification store (web UI, admin-only).
 *
 * Wraps `ApiClient.ServerUpdate.getStatus()` — the server polls GitHub daily
 * and caches the result; this store just reads it. NOTIFICATION ONLY: there is
 * no download/install (the server is updated manually via install.sh).
 */

import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import { type StoreProxy } from '@/core/stores'

const DISMISSED_VERSION_KEY = 'ziee:server-update:dismissed-version'

interface ServerUpdateState {
  currentVersion: string | null
  latestVersion: string | null
  updateAvailable: boolean
  releaseUrl: string | null
  notes: string | null
  /** Whether checks are enabled in server config (false → air-gapped). */
  enabled: boolean
  checkedAt: string | null
  /** "Dismiss" hides the banner for this session (resets on reload). */
  dismissed: boolean
  loading: boolean
  error: string | null

  // Key MUST be a state property the surfaces actually read — the store proxy
  // only fires `__init__[prop]` when `prop` is first accessed. Both the banner
  // and the About page read `updateAvailable`, so it hydrates for either.
  __init__: {
    updateAvailable: () => Promise<void>
  }

  loadStatus: () => Promise<void>
  dismiss: () => void
}

declare module '@/core/stores' {
  interface RegisteredStores {
    ServerUpdate: StoreProxy<ServerUpdateState>
  }
}

export const useServerUpdateStore = create<ServerUpdateState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      currentVersion: null,
      latestVersion: null,
      updateAvailable: false,
      releaseUrl: null,
      notes: null,
      enabled: true,
      checkedAt: null,
      dismissed: false,
      loading: false,
      error: null,

      __init__: {
        updateAvailable: async () => {
          // Eager-load via loadStatus so the About page surfaces any error
          // (a non-admin never reaches here — the banner is <Can>-gated and the
          // route is permission-gated, so neither surface mounts the store).
          await get().loadStatus()
        },
      },

      loadStatus: async () => {
        set((st) => {
          st.loading = true
          st.error = null
        })
        try {
          const s = await ApiClient.ServerUpdate.getStatus(undefined, undefined)
          set((st) => {
            st.currentVersion = s.current_version
            st.latestVersion = s.latest_version ?? null
            st.updateAvailable = s.update_available
            st.releaseUrl = s.release_url ?? null
            st.notes = s.notes ?? null
            st.enabled = s.enabled
            st.checkedAt = s.checked_at ?? null
            const dismissedVersion = localStorage.getItem(DISMISSED_VERSION_KEY)
            st.dismissed = s.update_available && !!dismissedVersion && dismissedVersion === (s.latest_version ?? null)
            st.loading = false
          })
        } catch (e) {
          set((st) => {
            st.loading = false
            st.error = e instanceof Error ? e.message : 'Failed to load update status'
          })
        }
      },

      dismiss: () => {
        const v = get().latestVersion
        if (v) { localStorage.setItem(DISMISSED_VERSION_KEY, v) }
        set((st) => {
          st.dismissed = true
        })
      },
    })),
  ),
)
