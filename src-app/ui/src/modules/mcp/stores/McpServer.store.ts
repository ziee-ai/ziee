import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type McpServer,
  type McpServerWithHealthWarning,
  type CreateMcpServerRequest,
  type UpdateMcpServerRequest,
  type McpServerOAuthConfigResponse,
  type SetMcpServerOAuthConfigRequest,
  type TestMcpConnectionRequest,
  type TestMcpConnectionResponse,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/SystemMcpServer.store'
import {
  emitMcpServerCreated,
  emitMcpServerUpdated,
  emitMcpServerDeleted,
} from '@/modules/mcp/events'
import { Stores } from '@/core/stores'

// Enable Map and Set support in Immer
enableMapSet()

/** Debounce timer for search-term reloads (250ms). */
let mcpSearchDebounce: ReturnType<typeof setTimeout> | null = null

interface McpState {
  // Server data (accessible servers - personal + system from groups)
  servers: McpServer[]
  isInitialized: boolean

  // Pagination state — drives the settings page's <Pagination> control.
  // Backend `listAccessible` accepts `page` + `per_page` and returns
  // `{ servers, total, page, per_page, total_pages }`.
  currentPage: number
  pageSize: number
  total: number

  // Server-side filter state. Search is debounced via the
  // setSearchTerm action; status changes fire an immediate reload.
  searchTerm: string
  statusFilter: string // 'all' | 'enabled' | 'disabled' | 'system' | 'user'

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Operation-specific loading states
  operationsLoading: Map<string, boolean>

  // Error states
  error: string | null

  // Initialization methods
  __init__: {
    __store__?: () => void
    servers: () => Promise<void>
  }

  __destroy__?: () => void

  // Actions
  loadMcpServers: (page?: number, pageSize?: number) => Promise<void>
  setSearchTerm: (q: string) => void
  setStatusFilter: (status: string) => void
  createMcpServer: (
    data: CreateMcpServerRequest,
  ) => Promise<McpServerWithHealthWarning>
  updateMcpServer: (
    serverId: string,
    data: UpdateMcpServerRequest,
  ) => Promise<McpServer>
  deleteMcpServer: (serverId: string) => Promise<void>
  getMcpServer: (serverId: string) => Promise<McpServer>
  getMcpServerOAuthConfig: (
    serverId: string,
  ) => Promise<McpServerOAuthConfigResponse | null>
  setMcpServerOAuthConfig: (
    serverId: string,
    config: SetMcpServerOAuthConfigRequest,
  ) => Promise<void>
  deleteMcpServerOAuthConfig: (serverId: string) => Promise<void>
  testMcpServerConnection: (
    data: TestMcpConnectionRequest,
  ) => Promise<TestMcpConnectionResponse>
  clearMcpError: () => void
  getUserServers: (servers: McpServer[]) => McpServer[]
  getSystemServers: (servers: McpServer[]) => McpServer[]
  getEnabledServers: (servers: McpServer[]) => McpServer[]
  getServersByType: (servers: McpServer[], transportType: string) => McpServer[]
  searchServers: (servers: McpServer[], query: string) => McpServer[]
}

export const useMcpStore = create<McpState>()(
  subscribeWithSelector(
    immer(
      (set, get): McpState => ({
        // Server data
        servers: [],
        isInitialized: false,

        // Pagination state (defaults match the settings page's
        // pageSizeOptions={['5','10','20','50']}).
        currentPage: 1,
        pageSize: 10,
        total: 0,

        // Filter state (server-side).
        searchTerm: '',
        statusFilter: 'all',

        // Loading states
        loading: false,
        creating: false,
        updating: false,
        deleting: false,

        // Operation-specific loading states
        operationsLoading: new Map<string, boolean>(),

        // Error states
        error: null,

        // Initialization methods
        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'McpServerStore'

            // Subscribe to mcp_server.created (user-owned servers only)
            eventBus.on(
              'mcp_server.created',
              async event => {
                const { server } = event.data
                // Skip system servers — they arrive via group reload
                if (!server.is_system) {
                  set(draft => {
                    draft.servers.push(server)
                  })
                }
              },
              GROUP,
            )

            // Subscribe to mcp_server.updated
            eventBus.on(
              'mcp_server.updated',
              async event => {
                const { server } = event.data
                set(draft => {
                  const index = draft.servers.findIndex(s => s.id === server.id)
                  if (index !== -1) {
                    draft.servers[index] = server
                  }
                })
              },
              GROUP,
            )

            // Subscribe to mcp_server.deleted
            eventBus.on(
              'mcp_server.deleted',
              async event => {
                const { serverId } = event.data
                set(draft => {
                  draft.servers = draft.servers.filter(s => s.id !== serverId)
                })
              },
              GROUP,
            )

            // Subscribe to mcp_server.groups_changed
            eventBus.on(
              'mcp_server.groups_changed',
              async () => {
                // Reload servers list to get fresh accessible servers
                await get().loadMcpServers()
              },
              GROUP,
            )

            // Subscribe to mcp_server.group_servers_changed (emitted by group assignment widget)
            eventBus.on(
              'mcp_server.group_servers_changed',
              async () => {
                await get().loadMcpServers()
              },
              GROUP,
            )

            // Subscribe to group.member_added
            eventBus.on(
              'group.member_added',
              async () => {
                // Reload servers list (user might gain access to system servers)
                await get().loadMcpServers()
              },
              GROUP,
            )

            // Subscribe to group.member_removed
            eventBus.on(
              'group.member_removed',
              async () => {
                // Reload servers list (user might lose access to system servers)
                await get().loadMcpServers()
              },
              GROUP,
            )
          },
          servers: () => get().loadMcpServers(),
        },

        // Actions
        loadMcpServers: async (page?: number, pageSize?: number): Promise<void> => {
          // Permission-gate the shell-eager-load fetch (audit
          // follow-up): AppLayout triggers this store's __init__ on
          // every render regardless of route, and for users without
          // mcp_servers::read the request 403s. Silently skip
          // instead — the rest of the app shell shouldn't show
          // a corresponding UI surface for these users anyway.
          if (!hasPermissionNow(Permissions.McpServersRead)) return

          const state = get()

          // Only prevent concurrent loads, not repeated ones
          if (state.loading) {
            return
          }

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
              ...(state.statusFilter !== 'all'
                ? { status: state.statusFilter }
                : {}),
            })

            set(draft => {
              draft.servers = response.servers
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
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to load MCP servers'
            })
            throw error
          }
        },

        // Filter setters — both reset to page 1 and reload. Search
        // is debounced so keystrokes coalesce into a single backend
        // hit; status changes fire immediately.
        setSearchTerm: (q: string) => {
          set(draft => {
            draft.searchTerm = q
            draft.currentPage = 1
          })
          if (mcpSearchDebounce) clearTimeout(mcpSearchDebounce)
          mcpSearchDebounce = setTimeout(() => {
            void get().loadMcpServers(1)
          }, 250)
        },
        setStatusFilter: (status: string) => {
          set(draft => {
            draft.statusFilter = status
            draft.currentPage = 1
          })
          void get().loadMcpServers(1)
        },

        createMcpServer: async (
          data: CreateMcpServerRequest,
        ): Promise<McpServerWithHealthWarning> => {
          try {
            set(draft => {
              draft.creating = true
              draft.error = null
            })

            // Response is flattened: the McpServer fields are at the
            // top level, with an optional `connection_warning` sibling
            // that appears only when the post-create probe failed and
            // the row was auto-downgraded. Caller (the drawer)
            // surfaces the warning toast — here we just emit downstream
            // off the canonical row.
            const wrapped = await ApiClient.McpServer.create(data)

            // Emit event after successful API call. Strip the warning
            // first so listeners receive a plain McpServer shape.
            const { connection_warning: _w, ...newServer } = wrapped
            try {
              await emitMcpServerCreated(newServer)
            } catch (eventError) {
              console.error(
                'Failed to emit mcp server created event:',
                eventError,
              )
            }

            set({ creating: false })

            // Surface the wrapper so the drawer can toast the
            // `connection_warning` if present.
            return wrapped
          } catch (error) {
            console.error('MCP server creation failed:', error)
            set(draft => {
              draft.creating = false
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to create MCP server'
            })
            throw error
          }
        },

        updateMcpServer: async (
          serverId: string,
          data: UpdateMcpServerRequest,
        ): Promise<McpServer> => {
          // Set loading for specific server
          set(draft => {
            draft.operationsLoading.set(serverId, true)
            draft.error = null
          })

          try {
            const updatedServer = await ApiClient.McpServer.update({
              id: serverId,
              ...data,
            })

            // Emit event after successful API call
            // Event handler will update state (no manual state update here)
            try {
              await emitMcpServerUpdated(updatedServer)
            } catch (eventError) {
              console.error(
                'Failed to emit mcp server updated event:',
                eventError,
              )
            }

            // Clear operation loading state
            set(draft => {
              draft.operationsLoading.delete(serverId)
            })

            // Update system MCP servers store if server exists there (doesn't use immer)
            useSystemMcpServersStore.setState(state => {
              const index = state.systemServers.findIndex(
                server => server.id === updatedServer.id,
              )
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
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to update MCP server'
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

            // Emit event after successful API call
            // Event handler will update state (no manual state update here)
            try {
              await emitMcpServerDeleted(serverId)
            } catch (eventError) {
              console.error(
                'Failed to emit mcp server deleted event:',
                eventError,
              )
            }

            // Clear operation loading state
            set(draft => {
              draft.operationsLoading.delete(serverId)
            })

            // Remove from system MCP servers store if it exists there (doesn't use immer)
            useSystemMcpServersStore.setState(state => ({
              ...state,
              systemServers: state.systemServers.filter(
                server => server.id !== serverId,
              ),
            }))
          } catch (error) {
            console.error('MCP server deletion failed:', error)
            set(draft => {
              draft.operationsLoading.delete(serverId)
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to delete MCP server'
            })
            throw error
          }
        },

        getMcpServer: async (serverId: string): Promise<McpServer> => {
          try {
            const server = await ApiClient.McpServer.get({ id: serverId })

            // Update server in main store
            set(draft => {
              const index = draft.servers.findIndex(s => s.id === server.id)
              if (index >= 0) {
                draft.servers[index] = server
              }
            })

            // Update system MCP servers store if server exists there
            useSystemMcpServersStore.setState(state => {
              const index = state.systemServers.findIndex(
                s => s.id === server.id,
              )
              if (index >= 0) {
                return {
                  ...state,
                  systemServers: state.systemServers.map(s =>
                    s.id === server.id ? server : s,
                  ),
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

        getMcpServerOAuthConfig: async (serverId: string) => {
          return await ApiClient.McpServer.getOAuthConfig({ id: serverId })
        },

        setMcpServerOAuthConfig: async (
          serverId: string,
          config: SetMcpServerOAuthConfigRequest,
        ) => {
          await ApiClient.McpServer.setOAuthConfig({
            id: serverId,
            ...config,
          })
        },

        deleteMcpServerOAuthConfig: async (serverId: string) => {
          await ApiClient.McpServer.deleteOAuthConfig({ id: serverId })
        },

        // Probe a candidate config (read-only; nothing is persisted). The
        // backend returns { success, message, tool_count } with HTTP 200 even
        // on a failed connection, so callers branch on `success`.
        testMcpServerConnection: async (
          data: TestMcpConnectionRequest,
        ): Promise<TestMcpConnectionResponse> => {
          return await ApiClient.McpServer.testConnection(data)
        },

        clearMcpError: () => {
          set(draft => {
            draft.error = null
          })
        },

        // Helper functions
        getUserServers: (servers: McpServer[]): McpServer[] => {
          return servers.filter(server => !server.is_system)
        },

        getSystemServers: (servers: McpServer[]): McpServer[] => {
          return servers.filter(server => server.is_system)
        },

        getEnabledServers: (servers: McpServer[]): McpServer[] => {
          return servers.filter(server => server.enabled)
        },

        getServersByType: (
          servers: McpServer[],
          transportType: string,
        ): McpServer[] => {
          return servers.filter(
            server =>
              server.transport_type.toLowerCase() ===
              transportType.toLowerCase(),
          )
        },

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

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('McpServerStore')
        },
      }),
    ),
  ),
)
