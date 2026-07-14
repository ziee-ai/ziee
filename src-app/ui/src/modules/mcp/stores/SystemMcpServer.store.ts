import { ApiClient } from '@/api-client'
import {
  type CreateMcpServerRequest,
  type McpServer,
  type McpServerWithHealthWarning,
  Permissions,
  type TestMcpConnectionRequest,
  type TestMcpConnectionResponse,
  type UpdateMcpServerRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitGroupSystemMcpServersChanged,
  emitMcpServerCreated,
  emitMcpServerDeleted,
  emitMcpServerUpdated,
} from '@/modules/mcp/events'

/** Debounce timer for system MCP search-term reloads (250ms). */
let sysMcpSearchDebounce: ReturnType<typeof setTimeout> | null = null

const INITIAL = {
  systemServers: [] as McpServer[],
  systemServersTotal: 0,
  systemServersPage: 1,
  systemServersPageSize: 20,
  systemServersInitialized: false,
  // Filter state (server-side). Search is debounced; status fires immediately.
  searchTerm: '',
  statusFilter: 'all',
  systemServersLoading: false,
  creating: false,
  updating: false,
  deleting: false,
  operationsLoading: new Map<string, boolean>(),
  systemServersError: null as string | null,
}

export const SystemMcpServer = defineStore('SystemMcpServer', {
  state: {
    ...INITIAL,
    // Wait 10s before destroying (users might come back). Read by the proxy.
    __destroyDelay__: 10000,
  },
  actions: (set, get) => {
    const loadSystemServers = async (page?: number, pageSize?: number): Promise<void> => {
      const state = get()
      if (state.systemServersInitialized && state.systemServersLoading && !page) return
      try {
        const requestPage = page || state.systemServersPage
        const requestPageSize = pageSize || state.systemServersPageSize
        set({ systemServersLoading: true, systemServersError: null })
        const response = await ApiClient.McpServerSystem.list({
          page: requestPage,
          per_page: requestPageSize,
          ...(state.searchTerm ? { search: state.searchTerm } : {}),
          ...(state.statusFilter !== 'all' ? { status: state.statusFilter } : {}),
        })
        set({
          systemServers: response.servers,
          systemServersTotal: response.total,
          systemServersPage: response.page,
          systemServersPageSize: response.per_page,
          systemServersInitialized: true,
          systemServersLoading: false,
          systemServersError: null,
        })
      } catch (error) {
        console.error('Failed to load system servers:', error)
        set({
          systemServersLoading: false,
          systemServersError:
            error instanceof Error ? error.message : 'Failed to load system servers',
        })
        throw error
      }
    }
    return {
      loadSystemServers,
      // Filter setters — both reset to page 1 and reload. Search debounced;
      // status fires immediately.
      setSearchTerm: (q: string) => {
        set({ searchTerm: q, systemServersPage: 1 })
        if (sysMcpSearchDebounce) clearTimeout(sysMcpSearchDebounce)
        sysMcpSearchDebounce = setTimeout(() => {
          void loadSystemServers(1)
        }, 250)
      },
      setStatusFilter: (status: string) => {
        set({ statusFilter: status, systemServersPage: 1 })
        void loadSystemServers(1)
      },
      createSystemServer: async (
        data: CreateMcpServerRequest,
      ): Promise<McpServerWithHealthWarning> => {
        try {
          set({ creating: true, systemServersError: null })
          // Response is flattened: McpServer fields at top level + optional
          // `connection_warning` sibling (health-check-on-create).
          const wrapped = await ApiClient.McpServerSystem.create(data)
          const { connection_warning: _w, ...newServer } = wrapped
          try {
            await emitMcpServerCreated(newServer)
          } catch (eventError) {
            console.error('Failed to emit mcp server created event:', eventError)
          }
          set(state => ({
            systemServers: [...state.systemServers, newServer],
            systemServersTotal: state.systemServersTotal + 1,
            creating: false,
          }))
          return wrapped
        } catch (error) {
          console.error('Failed to create system server:', error)
          set({
            creating: false,
            systemServersError:
              error instanceof Error ? error.message : 'Failed to create system server',
          })
          throw error
        }
      },
      updateSystemServer: async (id: string, data: UpdateMcpServerRequest): Promise<McpServer> => {
        try {
          set({ updating: true, systemServersError: null })
          const updatedServer = await ApiClient.McpServerSystem.update({ id, ...data })
          try {
            await emitMcpServerUpdated(updatedServer)
          } catch (eventError) {
            console.error('Failed to emit mcp server updated event:', eventError)
          }
          set(state => ({
            systemServers: state.systemServers.map(server =>
              server.id === id ? updatedServer : server,
            ),
            updating: false,
          }))
          return updatedServer
        } catch (error) {
          console.error('Failed to update system server:', error)
          set({
            updating: false,
            systemServersError:
              error instanceof Error ? error.message : 'Failed to update system server',
          })
          throw error
        }
      },
      deleteSystemServer: async (id: string): Promise<void> => {
        try {
          set({ deleting: true, systemServersError: null })
          await ApiClient.McpServerSystem.delete({ id })
          try {
            await emitMcpServerDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit mcp server deleted event:', eventError)
          }
          set(state => ({
            systemServers: state.systemServers.filter(server => server.id !== id),
            systemServersTotal: state.systemServersTotal - 1,
            deleting: false,
          }))
        } catch (error) {
          console.error('Failed to delete system server:', error)
          set({
            deleting: false,
            systemServersError:
              error instanceof Error ? error.message : 'Failed to delete system server',
          })
          throw error
        }
      },
      // Probe a candidate config (read-only; nothing persisted). 200 even on failure.
      testSystemServerConnection: async (
        data: TestMcpConnectionRequest,
      ): Promise<TestMcpConnectionResponse> => await ApiClient.McpServerSystem.testConnection(data),
      getServerGroups: async (serverId: string): Promise<string[]> => {
        try {
          return await ApiClient.McpServerSystem.getServerGroups({ id: serverId })
        } catch (error) {
          console.error('Failed to get server groups:', error)
          throw error
        }
      },
      assignServerToGroups: async (serverId: string, groupIds: string[]): Promise<void> => {
        try {
          await ApiClient.McpServerSystem.assignServerToGroups({ id: serverId, group_ids: groupIds })
        } catch (error) {
          console.error('Failed to assign server to groups:', error)
          throw error
        }
      },
      removeServerFromGroup: async (serverId: string, groupId: string): Promise<void> => {
        try {
          await ApiClient.McpServerSystem.removeServerFromGroup({ id: serverId, group_id: groupId })
        } catch (error) {
          console.error('Failed to remove server from group:', error)
          throw error
        }
      },
      updateGroupServers: async (groupId: string, serverIds: string[]): Promise<void> => {
        try {
          // Group-centric bulk update endpoint.
          await ApiClient.Group.updateSystemServers({ group_id: groupId, server_ids: serverIds })
          await emitGroupSystemMcpServersChanged(groupId, serverIds)
        } catch (error) {
          console.error('Failed to update group servers:', error)
          throw error
        }
      },
      getServersForGroup: async (groupId: string): Promise<McpServer[]> => {
        try {
          // Read the group's assigned servers directly from the canonical
          // endpoint (iterating the paginated cache dropped servers not in it).
          const response = await ApiClient.Group.getSystemServers({ group_id: groupId })
          // Guard: callers `.map` the result — never hand back undefined.
          return Array.isArray(response.servers) ? response.servers : []
        } catch (error) {
          console.error('Failed to get servers for group:', error)
          throw error
        }
      },
      clearSystemMcpErrors: () => {
        set({ systemServersError: null })
      },
      refreshSystemServers: async (): Promise<void> => {
        const { systemServersPage, systemServersPageSize } = get()
        await loadSystemServers(systemServersPage, systemServersPageSize)
      },
      isServerOperationLoading: (serverId: string, operation?: string): boolean => {
        const { operationsLoading } = get()
        const operationKey = operation ? `${serverId}-${operation}` : serverId
        return operationsLoading.get(operationKey) || false
      },
      getSystemServerById: (serverId: string): McpServer | null =>
        get().systemServers.find(server => server.id === serverId) || null,
      getEnabledSystemServers: (): McpServer[] =>
        get().systemServers.filter(server => server.enabled),
      searchSystemServers: (query: string): McpServer[] => {
        const { systemServers } = get()
        if (!query.trim()) return systemServers
        const searchTerm = query.toLowerCase()
        return systemServers.filter(
          server =>
            server.name.toLowerCase().includes(searchTerm) ||
            server.display_name.toLowerCase().includes(searchTerm) ||
            server.description?.toLowerCase().includes(searchTerm) ||
            server.transport_type.toLowerCase().includes(searchTerm),
        )
      },
    }
  },
  init: ({ on, set, actions, onCleanup }) => {
    on('mcp_server.created', event => {
      const { server } = event.data
      if (server.is_system) {
        set(state => ({
          systemServers: [...state.systemServers, server],
          systemServersTotal: state.systemServersTotal + 1,
        }))
      }
    })
    on('mcp_server.updated', event => {
      const { server } = event.data
      if (server.is_system) {
        set(state => ({
          systemServers: state.systemServers.map(s => (s.id === server.id ? server : s)),
        }))
      }
    })
    on('mcp_server.deleted', event => {
      set(state => ({
        systemServers: state.systemServers.filter(s => s.id !== event.data.serverId),
        systemServersTotal: state.systemServersTotal - 1,
      }))
    })
    // Cross-device sync for the admin system (deployment-shared) table. Self-gate
    // on mcp_servers_admin::read — loadSystemServers does NOT gate internally.
    const reload = () => {
      if (!hasPermissionNow(Permissions.McpServersAdminRead)) return
      void actions.loadSystemServers()
    }
    on('sync:mcp_server_system', reload)
    on('sync:reconnect', reload)
    void actions.loadSystemServers()
    // Reset to initial state on destroy so a re-mount starts clean + refetches.
    onCleanup(() => {
      set({ ...INITIAL, operationsLoading: new Map() })
    })
  },
})

export const useSystemMcpServersStore = SystemMcpServer.store
