import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubMCPServer } from '@/api-client/types'

interface HubMcpServersState {
  servers: HubMCPServer[]
  version: string | null
  loading: boolean
  error: string | null

  // Actions
  loadServers: () => Promise<void>
  refreshFromGitHub: () => Promise<void>

  // Lazy initialization
  __init__: {
    servers: () => Promise<void>
  }
}

export const useHubMcpServersStore = create<HubMcpServersState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubMcpServersState => ({
        servers: [],
        version: null,
        loading: false,
        error: null,

        loadServers: async () => {
          const state = get()
          if (state.loading) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const servers = await ApiClient.Hub.getMCPServers({ lang: locale })
            const versionInfo = await ApiClient.Hub.getMCPServersVersion()

            set({
              servers,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub MCP servers',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshMCPServers()

            // Reload if updated
            if (result.updated) {
              await get().loadServers()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub MCP servers',
              loading: false,
            })
            throw error
          }
        },

        __init__: {
          servers: () => get().loadServers(),
        },
      })
    )
  )
)
