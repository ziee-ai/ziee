import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type ConnectorCatalogEntry,
  type LitSearchSettings,
  type UpdateConnectorRequest,
  type UpdateLitSearchSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface LitSearchAdminStore {
  settings: LitSearchSettings | null
  connectors: ConnectorCatalogEntry[]
  loading: boolean
  savingSettings: boolean
  /** Connector key being saved, or null (scopes the spinner to one form). */
  savingConnector: string | null
  error: string | null

  __init__: {
    __store__?: () => void
    settings: () => Promise<void>
    connectors: () => Promise<void>
  }
  __destroy__?: () => void

  load: () => Promise<void>
  updateSettings: (patch: UpdateLitSearchSettingsRequest) => Promise<LitSearchSettings>
  updateConnector: (connector: string, body: UpdateConnectorRequest) => Promise<void>
}

const loadSettings = async (set: (fn: (s: LitSearchAdminStore) => void) => void) => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.LitSearch.getSettings()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error =
        error instanceof Error ? error.message : 'Failed to load literature search settings'
      s.loading = false
    })
  }
}

const loadConnectors = async (set: (fn: (s: LitSearchAdminStore) => void) => void) => {
  try {
    const res = await ApiClient.LitSearch.getConnectors()
    set(s => {
      s.connectors = res.connectors
    })
  } catch (error) {
    set(s => {
      s.error = error instanceof Error ? error.message : 'Failed to load connectors'
    })
  }
}

export const useLitSearchAdminStore = create<LitSearchAdminStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      settings: null,
      connectors: [],
      loading: false,
      savingSettings: false,
      savingConnector: null,
      error: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'LitSearchAdmin'
          const reload = () => {
            if (!hasPermissionNow(Permissions.LitSearchAdminRead)) return
            void loadSettings(set)
            void loadConnectors(set)
          }
          eventBus.on('sync:lit_search_settings', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        settings: () =>
          hasPermissionNow(Permissions.LitSearchAdminRead)
            ? loadSettings(set)
            : Promise.resolve(),
        connectors: () =>
          hasPermissionNow(Permissions.LitSearchAdminRead)
            ? loadConnectors(set)
            : Promise.resolve(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LitSearchAdmin')
      },

      load: async () => {
        await Promise.all([loadSettings(set), loadConnectors(set)])
      },

      updateSettings: async (patch): Promise<LitSearchSettings> => {
        set(s => {
          s.savingSettings = true
          s.error = null
        })
        try {
          const row = await ApiClient.LitSearch.updateSettings(patch)
          set(s => {
            s.settings = row
            s.savingSettings = false
          })
          return row
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.savingSettings = false
          })
          throw error
        }
      },

      updateConnector: async (connector, body): Promise<void> => {
        set(s => {
          s.savingConnector = connector
          s.error = null
        })
        try {
          const res = await ApiClient.LitSearch.updateConnector({ connector, ...body })
          set(s => {
            s.connectors = res.connectors
            s.savingConnector = null
          })
        } catch (error) {
          set(s => {
            s.error = error instanceof Error ? error.message : 'Update failed'
            s.savingConnector = null
          })
          throw error
        }
      },
    })),
  ),
)
