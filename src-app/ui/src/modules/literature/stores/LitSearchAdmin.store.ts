import { ApiClient } from '@/api-client'
import {
  type ConnectorCatalogEntry,
  type LitSearchSettings,
  Permissions,
  type UpdateConnectorRequest,
  type UpdateLitSearchSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

export const LitSearchAdmin = defineStore('LitSearchAdmin', {
  immer: true,
  state: {
    settings: null as LitSearchSettings | null,
    connectors: [] as ConnectorCatalogEntry[],
    loading: false,
    savingSettings: false,
    /** Connector key being saved, or null (scopes the spinner to one form). */
    savingConnector: null as string | null,
    error: null as string | null,
  },
  actions: set => {
    const loadSettings = async () => {
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
            error instanceof Error
              ? error.message
              : 'Failed to load literature search settings'
          s.loading = false
        })
      }
    }
    const loadConnectors = async () => {
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
    return {
      loadSettings,
      loadConnectors,
      load: async () => {
        await Promise.all([loadSettings(), loadConnectors()])
      },
      updateSettings: async (
        patch: UpdateLitSearchSettingsRequest,
      ): Promise<LitSearchSettings> => {
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
      updateConnector: async (
        connector: string,
        body: UpdateConnectorRequest,
      ): Promise<void> => {
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
    }
  },
  init: ({ on, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.LitSearchAdminRead)) return
      void actions.loadSettings()
      void actions.loadConnectors()
    }
    on('sync:lit_search_settings', reload)
    on('sync:reconnect', reload)
    reload()
  },
})

export const useLitSearchAdminStore = LitSearchAdmin.store
