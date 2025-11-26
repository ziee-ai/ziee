import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  HubMCPServer,
  McpServer,
  CreateMcpServerFromHubRequest,
} from '@/api-client/types'

interface HubMcpServersState {
  servers: HubMCPServer[]
  version: string | null
  loading: boolean
  creating: boolean
  error: string | null

  // Actions
  loadServers: () => Promise<void>
  refreshFromGitHub: () => Promise<void>
  createFromHub: (request: CreateMcpServerFromHubRequest) => Promise<McpServer>

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
        creating: false,
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

        createFromHub: async (
          request: CreateMcpServerFromHubRequest,
        ): Promise<McpServer> => {
          set({ creating: true, error: null })
          try {
            const response = await ApiClient.Hub.createMcpServerFromHub(request)

            // Update the hub MCP server's created_ids directly from response
            set(state => {
              const server = state.servers.find(s => s.id === request.hub_id)
              if (server) {
                if (!server.created_ids) {
                  server.created_ids = []
                }
                server.created_ids.push(response.hub_tracking.entity_id)
              }
              state.creating = false
            })

            return response.server
          } catch (error: any) {
            set({
              error: error.message || 'Failed to create MCP server from hub',
              creating: false,
            })
            throw error
          }
        },

        __init__: {
          servers: () => get().loadServers(),
        },
      }),
    ),
  ),
)
