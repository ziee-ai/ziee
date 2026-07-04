import { ApiClient } from '@/api-client'
import type {
  CreateMcpServerFromHubRequest,
  HubMCPServer,
  McpServer,
} from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import { emitMcpServerCreated, emitMcpServerDeleted } from '@/modules/mcp/events'

export const HubMcpServers = defineStore('HubMcpServers', {
  immer: true,
  state: {
    servers: [] as HubMCPServer[],
    version: null as string | null,
    loading: false,
    creating: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadServers = async (force = false) => {
      if (get().loading && !force) return
      set({ loading: true, error: null })
      try {
        const locale = 'en' // TODO: Get from user settings
        const servers = await ApiClient.Hub.getMCPServers({ lang: locale })
        const versionInfo = await ApiClient.Hub.getMCPServersVersion()
        set({ servers, version: versionInfo.version, loading: false })
      } catch (error: any) {
        set({ error: error.message || 'Failed to load hub MCP servers', loading: false })
      }
    }
    return {
      loadServers,
      refreshFromGitHub: async () => {
        set({ loading: true, error: null })
        try {
          const result = await ApiClient.Hub.refreshMCPServers()
          if (result.updated) await loadServers()
          set({ loading: false })
        } catch (error: any) {
          set({ error: error.message || 'Failed to refresh hub MCP servers', loading: false })
          throw error
        }
      },
      createFromHub: async (request: CreateMcpServerFromHubRequest): Promise<McpServer> => {
        set({ creating: true, error: null })
        // Snapshot displaced ids BEFORE the call so the `replace_existing` path
        // can emit `mcp_server.deleted` for them after the new row exists.
        const displacedIds: string[] = request.replace_existing
          ? (get().servers.find(s => s.name === request.hub_id)?.created_ids?.slice() ?? [])
          : []
        try {
          const response = await ApiClient.Hub.createMcpServerFromHub(request)
          set(state => {
            const server = state.servers.find(s => s.name === request.hub_id)
            if (server) {
              if (request.replace_existing) {
                server.created_ids = [response.hub_tracking.entity_id]
              } else {
                if (!server.created_ids) server.created_ids = []
                server.created_ids.push(response.hub_tracking.entity_id)
              }
            }
            state.creating = false
          })
          for (const oldId of displacedIds) {
            if (oldId !== response.hub_tracking.entity_id) {
              try {
                await emitMcpServerDeleted(oldId)
              } catch (e) {
                console.warn('Failed to emit mcp_server.deleted:', e)
              }
            }
          }
          try {
            await emitMcpServerCreated(response.server)
          } catch (e) {
            console.warn('Failed to emit mcp_server.created:', e)
          }
          return response.server
        } catch (error: any) {
          set({ error: error.message || 'Failed to create MCP server from hub', creating: false })
          throw error
        }
      },
      /** Install as a system-wide MCP server (is_system=true). Backend requires
       *  `hub::mcp_servers::create` + `mcp_servers_admin::create`; the frontend
       *  gates on `McpServersAdminCreate`. `replace_existing` overrides the 409. */
      createSystemFromHub: async (
        request: CreateMcpServerFromHubRequest,
      ): Promise<McpServer> => {
        set({ creating: true, error: null })
        const displacedIds: string[] = request.replace_existing
          ? (get().servers.find(s => s.name === request.hub_id)?.created_system_ids?.slice() ?? [])
          : []
        try {
          const response = await ApiClient.Hub.createSystemMcpServerFromHub(request)
          set(state => {
            const server = state.servers.find(s => s.name === request.hub_id)
            if (server) {
              if (request.replace_existing) {
                server.created_system_ids = [response.hub_tracking.entity_id]
              } else {
                if (!server.created_system_ids) server.created_system_ids = []
                server.created_system_ids.push(response.hub_tracking.entity_id)
              }
            }
            state.creating = false
          })
          for (const oldId of displacedIds) {
            if (oldId !== response.hub_tracking.entity_id) {
              try {
                await emitMcpServerDeleted(oldId)
              } catch (e) {
                console.warn('Failed to emit mcp_server.deleted:', e)
              }
            }
          }
          try {
            await emitMcpServerCreated(response.server)
          } catch (e) {
            console.warn('Failed to emit mcp_server.created:', e)
          }
          return response.server
        } catch (error: any) {
          set({
            error: error.message || 'Failed to create system MCP server from hub',
            creating: false,
          })
          throw error
        }
      },
    }
  },
  init: ({ on, set, actions }) => {
    // One listener clears both user- and system-install tracking arrays —
    // backend doesn't discriminate scope on delete (single mcp_server.deleted).
    on('mcp_server.deleted', event => {
      const { serverId } = event.data
      set(state => {
        for (const server of state.servers) {
          if (server.created_ids) {
            server.created_ids = server.created_ids.filter(id => id !== serverId)
          }
          if (server.created_system_ids) {
            server.created_system_ids = server.created_system_ids.filter(id => id !== serverId)
          }
        }
      })
    })
    void actions.loadServers()
  },
})

export const useHubMcpServersStore = HubMcpServers.store
