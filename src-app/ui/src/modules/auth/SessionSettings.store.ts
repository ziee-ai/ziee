import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type SessionSettings,
  type UpdateSessionSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { type StoreProxy, Stores } from '@/core/stores'

/**
 * Admin view of the deployment-wide JWT session settings (singleton):
 * access-token TTL + max session length. Mirrors WebSearchAdmin.store —
 * self-gated loaders + `sync:session_settings` / `sync:reconnect`
 * subscriptions so another admin's edit refreshes this tab live.
 */
interface SessionSettingsStore {
  settings: SessionSettings | null
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    settings: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  update: (patch: UpdateSessionSettingsRequest) => Promise<SessionSettings>
}

declare module '../../core/stores' {
  interface RegisteredStores {
    SessionSettings: StoreProxy<SessionSettingsStore>
  }
}

const loadSettings = async (
  set: (fn: (s: SessionSettingsStore) => void) => void,
) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.Auth.getSessionSettings()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load session settings'
      s.loading = false
    })
  }
}

export const useSessionSettingsStore = create<SessionSettingsStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      loading: false,
      saving: false,
      error: null,

      // The loader hits the `auth::session_settings::read`-gated endpoint.
      // Self-gate so non-admins never generate 403s (incl. on the
      // audience-agnostic `sync:reconnect`).
      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'SessionSettings'
          const reload = () => {
            if (!hasPermissionNow(Permissions.SessionSettingsRead)) return
            void loadSettings(set)
          }
          eventBus.on('sync:session_settings', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        settings: () =>
          hasPermissionNow(Permissions.SessionSettingsRead)
            ? loadSettings(set)
            : Promise.resolve(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('SessionSettings')
      },

      load: async () => {
        await loadSettings(set)
      },

      update: async (patch): Promise<SessionSettings> => {
        set(s => {
          s.saving = true
          s.error = null
        })
        try {
          const row = await ApiClient.Auth.updateSessionSettings(patch)
          set(s => {
            s.settings = row
            s.saving = false
          })
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.saving = false
          })
          throw error
        }
      },
    })),
  ),
)
