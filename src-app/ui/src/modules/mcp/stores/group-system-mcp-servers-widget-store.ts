import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { McpServer } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

interface GroupServers {
  groupId: string
  servers: McpServer[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface GroupSystemMcpServersWidgetState {
  // Map of groupId -> server data
  groupServers: Map<string, GroupServers>

  // Cached servers
  allServers: McpServer[]
  serversLoading: boolean
  serversError: string | null
  serversInitialized: boolean

  // Initialization
  __init__: {
    __store__: () => void
    allServers: () => Promise<void>
  }

  // Actions (methods within store)
  loadAllServers: () => Promise<void>
  loadServersForGroup: (groupId: string, force?: boolean) => Promise<void>
  clearGroupServers: (groupId: string) => void
  clearAllGroupServers: () => void
  getGroupServersData: (groupId: string) => GroupServers | undefined
}

export const useGroupSystemMcpServersWidgetStore = create<GroupSystemMcpServersWidgetState>()(
  subscribeWithSelector(
    immer(
      (set, get): GroupSystemMcpServersWidgetState => ({
        groupServers: new Map(),
        allServers: [],
        serversLoading: false,
        serversError: null,
        serversInitialized: false,
        __init__: {
          // Store-level initialization - runs once on first access (any property)
          __store__: () => {
            // Subscribe to group server assignment changes
            const eventBus = Stores.EventBus

            // When servers are assigned to a group, update the cache directly
            eventBus.on('mcp_server.group_servers_changed', async event => {
              const { groupId, serverIds } = event.data

              // Ensure servers are loaded
              await get().loadAllServers()

              // Use cached servers to build the assigned list
              const allServers = get().allServers
              const assignedServers = allServers.filter(s => serverIds.includes(s.id))

              set(state => {
                state.groupServers.set(groupId, {
                  groupId,
                  servers: assignedServers,
                  loading: false,
                  error: null,
                  lastFetched: Date.now(),
                })
              })
            })

            // Subscribe to mcp_server.created
            eventBus.on('mcp_server.created', async event => {
              const { server } = event.data
              // Only handle system servers
              if (server.is_system) {
                set(state => {
                  state.serversInitialized = false
                })
                await get().loadAllServers()
              }
            })

            // Subscribe to mcp_server.updated
            eventBus.on('mcp_server.updated', async event => {
              const { server } = event.data
              // Only handle system servers
              if (server.is_system) {
                set(state => {
                  const index = state.allServers.findIndex(s => s.id === server.id)
                  if (index !== -1) {
                    state.allServers[index] = server
                  }
                })
              }
            })

            // Subscribe to mcp_server.deleted
            eventBus.on('mcp_server.deleted', async event => {
              const { serverId } = event.data
              set(state => {
                // Remove from allServers cache
                state.allServers = state.allServers.filter(s => s.id !== serverId)

                // Clear it from all groupServers maps
                state.groupServers.forEach((groupData, groupId) => {
                  const updatedServers = groupData.servers.filter(s => s.id !== serverId)
                  state.groupServers.set(groupId, {
                    ...groupData,
                    servers: updatedServers,
                  })
                })
              })
            })
          },

          // Property-specific initialization - runs when allServers is first accessed
          allServers: async () => {
            await get().loadAllServers()
          },
        },

        /**
         * Load all servers (cached)
         * Only fetches if not already initialized
         */
        loadAllServers: async (): Promise<void> => {
          const state = get()

          // If already loading, don't start another fetch
          if (state.serversLoading) {
            return
          }

          // If already initialized, use cached data
          if (state.serversInitialized && !state.serversError) {
            return
          }

          set(state => {
            state.serversLoading = true
            state.serversError = null
          })

          try {
            const response = await ApiClient.McpServerSystem.list({ page: 1, per_page: 1000 })

            set(state => {
              state.allServers = response.servers
              state.serversLoading = false
              state.serversError = null
              state.serversInitialized = true
            })
          } catch (error) {
            console.error('Failed to load servers:', error)

            set(state => {
              state.serversLoading = false
              state.serversError = error instanceof Error ? error.message : 'Failed to load servers'
            })

            throw error
          }
        },

        /**
         * Load servers for a specific group
         * Uses cached servers instead of fetching every time
         */
        loadServersForGroup: async (groupId: string, force = false): Promise<void> => {
          const state = get()
          const existing = state.groupServers.get(groupId)

          // If already loading, don't start another fetch
          if (existing?.loading && !force) {
            return
          }

          // If data is fresh (< 30 seconds old) and not forcing, use cached data
          if (
            !force &&
            existing?.lastFetched &&
            Date.now() - existing.lastFetched < 30000 &&
            !existing.error
          ) {
            return
          }

          // Set loading state
          set(state => {
            state.groupServers.set(groupId, {
              groupId,
              servers: existing?.servers || [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched || null,
            })
          })

          try {
            // Ensure servers are loaded
            await get().loadAllServers()

            // Get all cached servers
            const allServers = get().allServers

            // For each server, check if it's assigned to this group
            const assignedServers: McpServer[] = []
            for (const server of allServers) {
              const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: server.id })
              if (groupIds.includes(groupId)) {
                assignedServers.push(server)
              }
            }

            set(state => {
              state.groupServers.set(groupId, {
                groupId,
                servers: assignedServers,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(`Failed to load servers for group ${groupId}:`, error)

            set(state => {
              state.groupServers.set(groupId, {
                groupId,
                servers: existing?.servers || [],
                loading: false,
                error: error instanceof Error ? error.message : 'Failed to load servers',
                lastFetched: existing?.lastFetched || null,
              })
            })

            throw error
          }
        },

        /**
         * Clear cached data for a specific group
         */
        clearGroupServers: (groupId: string): void => {
          set(state => {
            state.groupServers.delete(groupId)
          })
        },

        /**
         * Clear all cached server data
         */
        clearAllGroupServers: (): void => {
          set(state => {
            state.groupServers.clear()
          })
        },

        /**
         * Get servers for a specific group from the store
         */
        getGroupServersData: (groupId: string): GroupServers | undefined => {
          return get().groupServers.get(groupId)
        },
      }),
    ),
  ),
)
