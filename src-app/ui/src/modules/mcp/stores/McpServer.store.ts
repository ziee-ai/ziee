import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  type CreateMcpServerRequest,
  type McpServer,
  type McpServerOAuthConfigResponse,
  type McpServerWithHealthWarning,
  Permissions,
  type SandboxFlavorsResponse,
  type SetMcpServerOAuthConfigRequest,
  type TestMcpConnectionRequest,
  type TestMcpConnectionResponse,
  type UpdateMcpServerRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitMcpServerCreated,
  emitMcpServerDeleted,
  emitMcpServerUpdated,
} from '@/modules/mcp/events'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/SystemMcpServer.store'

enableMapSet()

/** Debounce timer for search-term reloads (250ms). */
let mcpSearchDebounce: ReturnType<typeof setTimeout> | null = null

export const McpServerStoreDef = defineStore('McpServer', {
  immer: true,
  state: {
    // Accessible servers (personal + system from groups)
    servers: [] as McpServer[],
    isInitialized: false,
    // Pagination (defaults match the settings page's pageSizeOptions).
    currentPage: 1,
    pageSize: 10,
    total: 0,
    // Filter state (server-side). Search debounced; status immediate.
    searchTerm: '',
    statusFilter: 'all',
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    operationsLoading: new Map<string, boolean>(),
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadMcpServers = async (page?: number, pageSize?: number): Promise<void> => {
      // Permission-gate the shell-eager-load fetch: AppLayout triggers this
      // store's init on every render regardless of route; skip for users
      // without mcp_servers::read (the request would 403).
      if (!hasPermissionNow(Permissions.McpServersRead)) return
      const state = get()
      if (state.loading) return
      const nextPage = page ?? state.currentPage
      const nextPageSize = pageSize ?? state.pageSize
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.McpServer.listAccessible({
          page: nextPage,
          per_page: nextPageSize,
          ...(state.searchTerm ? { search: state.searchTerm } : {}),
          ...(state.statusFilter !== 'all' ? { status: state.statusFilter } : {}),
        })
        set(draft => {
          // Defensive: never assign a non-array (the list is iterated/`.length`-read).
          draft.servers = Array.isArray(response.servers) ? response.servers : []
          draft.total = response.total
          draft.currentPage = response.page
          draft.pageSize = response.per_page
          draft.isInitialized = true
          draft.loading = false
          draft.error = null
        })
      } catch (error) {
        console.error('MCP servers loading failed:', error)
        set(draft => {
          draft.loading = false
          draft.error = error instanceof Error ? error.message : 'Failed to load MCP servers'
        })
        throw error
      }
    }
    return {
      loadMcpServers,
      // Filter setters — reset to page 1 and reload. Search debounced; status immediate.
      setSearchTerm: (q: string) => {
        set(draft => {
          draft.searchTerm = q
          draft.currentPage = 1
        })
        if (mcpSearchDebounce) clearTimeout(mcpSearchDebounce)
        mcpSearchDebounce = setTimeout(() => {
          void loadMcpServers(1)
        }, 250)
      },
      setStatusFilter: (status: string) => {
        set(draft => {
          draft.statusFilter = status
          draft.currentPage = 1
        })
        void loadMcpServers(1)
      },
      createMcpServer: async (
        data: CreateMcpServerRequest,
      ): Promise<McpServerWithHealthWarning> => {
        try {
          set(draft => {
            draft.creating = true
            draft.error = null
          })
          // Flattened response: McpServer fields at top level + optional
          // `connection_warning` sibling (post-create probe auto-downgrade).
          const wrapped = await ApiClient.McpServer.create(data)
          const { connection_warning: _w, ...newServer } = wrapped
          try {
            await emitMcpServerCreated(newServer)
          } catch (eventError) {
            console.error('Failed to emit mcp server created event:', eventError)
          }
          set({ creating: false })
          return wrapped
        } catch (error) {
          console.error('MCP server creation failed:', error)
          set(draft => {
            draft.creating = false
            draft.error = error instanceof Error ? error.message : 'Failed to create MCP server'
          })
          throw error
        }
      },
      updateMcpServer: async (
        serverId: string,
        data: UpdateMcpServerRequest,
      ): Promise<McpServer> => {
        set(draft => {
          draft.operationsLoading.set(serverId, true)
          draft.error = null
        })
        try {
          const updatedServer = await ApiClient.McpServer.update({ id: serverId, ...data })
          try {
            await emitMcpServerUpdated(updatedServer)
          } catch (eventError) {
            console.error('Failed to emit mcp server updated event:', eventError)
          }
          set(draft => {
            draft.operationsLoading.delete(serverId)
          })
          // Mirror into the system store if the row lives there (plain, no immer).
          useSystemMcpServersStore.setState(state => {
            const index = state.systemServers.findIndex(server => server.id === updatedServer.id)
            if (index >= 0) {
              return {
                ...state,
                systemServers: state.systemServers.map(server =>
                  server.id === updatedServer.id ? updatedServer : server,
                ),
              }
            }
            return state
          })
          return updatedServer
        } catch (error) {
          console.error('MCP server update failed:', error)
          set(draft => {
            draft.operationsLoading.delete(serverId)
            draft.error = error instanceof Error ? error.message : 'Failed to update MCP server'
          })
          throw error
        }
      },
      deleteMcpServer: async (serverId: string): Promise<void> => {
        set(draft => {
          draft.operationsLoading.set(serverId, true)
          draft.error = null
        })
        try {
          await ApiClient.McpServer.delete({ id: serverId })
          try {
            await emitMcpServerDeleted(serverId)
          } catch (eventError) {
            console.error('Failed to emit mcp server deleted event:', eventError)
          }
          set(draft => {
            draft.operationsLoading.delete(serverId)
          })
          useSystemMcpServersStore.setState(state => ({
            ...state,
            systemServers: state.systemServers.filter(server => server.id !== serverId),
          }))
        } catch (error) {
          console.error('MCP server deletion failed:', error)
          set(draft => {
            draft.operationsLoading.delete(serverId)
            draft.error = error instanceof Error ? error.message : 'Failed to delete MCP server'
          })
          throw error
        }
      },
      getMcpServer: async (serverId: string): Promise<McpServer> => {
        try {
          const server = await ApiClient.McpServer.get({ id: serverId })
          set(draft => {
            const index = draft.servers.findIndex(s => s.id === server.id)
            if (index >= 0) draft.servers[index] = server
          })
          useSystemMcpServersStore.setState(state => {
            const index = state.systemServers.findIndex(s => s.id === server.id)
            if (index >= 0) {
              return {
                ...state,
                systemServers: state.systemServers.map(s => (s.id === server.id ? server : s)),
              }
            }
            return state
          })
          return server
        } catch (error) {
          console.error('Failed to get MCP server:', error)
          throw error
        }
      },
      getMcpServerOAuthConfig: async (
        serverId: string,
      ): Promise<McpServerOAuthConfigResponse | null> =>
        await ApiClient.McpServer.getOAuthConfig({ id: serverId }),
      // Lazily fetched by the system-server form to populate the sandbox flavor
      // picker. Admin-gated; only called from create-system/edit-system mode.
      getSandboxFlavors: async (): Promise<SandboxFlavorsResponse> =>
        await ApiClient.CodeSandbox.listFlavors(),
      setMcpServerOAuthConfig: async (
        serverId: string,
        config: SetMcpServerOAuthConfigRequest,
      ) => {
        await ApiClient.McpServer.setOAuthConfig({ id: serverId, ...config })
      },
      deleteMcpServerOAuthConfig: async (serverId: string) => {
        await ApiClient.McpServer.deleteOAuthConfig({ id: serverId })
      },
      // Probe a candidate config (read-only; nothing persisted). 200 even on failure.
      testMcpServerConnection: async (
        data: TestMcpConnectionRequest,
      ): Promise<TestMcpConnectionResponse> => await ApiClient.McpServer.testConnection(data),
      clearMcpError: () => {
        set(draft => {
          draft.error = null
        })
      },
      // Helper (pure) functions.
      getUserServers: (servers: McpServer[]): McpServer[] =>
        servers.filter(server => !server.is_system),
      getSystemServers: (servers: McpServer[]): McpServer[] =>
        servers.filter(server => server.is_system),
      getEnabledServers: (servers: McpServer[]): McpServer[] =>
        servers.filter(server => server.enabled),
      getServersByType: (servers: McpServer[], transportType: string): McpServer[] =>
        servers.filter(
          server => server.transport_type.toLowerCase() === transportType.toLowerCase(),
        ),
      searchServers: (servers: McpServer[], query: string): McpServer[] => {
        if (!query.trim()) return servers
        const searchTerm = query.toLowerCase()
        return servers.filter(
          server =>
            server.name.toLowerCase().includes(searchTerm) ||
            server.display_name.toLowerCase().includes(searchTerm) ||
            server.description?.toLowerCase().includes(searchTerm) ||
            server.transport_type.toLowerCase().includes(searchTerm),
        )
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('mcp_server.created', event => {
      const { server } = event.data
      // Skip system servers — they arrive via group reload.
      if (!server.is_system) {
        set(draft => {
          draft.servers.push(server)
        })
      }
    })
    on('mcp_server.updated', event => {
      const { server } = event.data
      set(draft => {
        const index = draft.servers.findIndex(s => s.id === server.id)
        if (index !== -1) draft.servers[index] = server
      })
    })
    on('mcp_server.deleted', event => {
      set(draft => {
        draft.servers = draft.servers.filter(s => s.id !== event.data.serverId)
      })
    })
    // Reload the accessible set on any visibility change.
    on('mcp_server.groups_changed', () => void actions.loadMcpServers())
    on('mcp_server.group_servers_changed', () => void actions.loadMcpServers())
    on('group.member_added', () => void actions.loadMcpServers())
    on('group.member_removed', () => void actions.loadMcpServers())
    // Cross-device sync. loadMcpServers is permission-gated internally.
    const reload = () => void actions.loadMcpServers()
    on('sync:mcp_server', reload)
    on('sync:user_mcp_server', reload)
    on('sync:reconnect', reload)
    void actions.loadMcpServers()
  },
})

export const useMcpStore = McpServerStoreDef.store
