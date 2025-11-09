import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Group } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

interface ServerGroups {
  serverId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

interface SystemMcpServerGroupCardState {
  // Map of serverId -> group data
  serverGroups: Map<string, ServerGroups>

  // Cached user groups
  allGroups: Group[]
  groupsLoading: boolean
  groupsError: string | null
  groupsInitialized: boolean

  // Initialization
  __init__: {
    __store__: () => void
    allGroups: () => Promise<void>
  }

  // Actions (methods within store)
  loadAllGroups: () => Promise<void>
  loadGroupsForServer: (serverId: string, force?: boolean) => Promise<void>
  clearServerGroups: (serverId: string) => void
  clearAllServerGroups: () => void
  getServerGroupsData: (serverId: string) => ServerGroups | undefined
}

export const useSystemMcpServerGroupCardStore = create<SystemMcpServerGroupCardState>()(
  subscribeWithSelector(
    immer(
      (set, get): SystemMcpServerGroupCardState => ({
        serverGroups: new Map(),
        allGroups: [],
        groupsLoading: false,
        groupsError: null,
        groupsInitialized: false,
        __init__: {
          // Store-level initialization - runs once on first access (any property)
          __store__: () => {
            // Subscribe to group events for cache invalidation
            const eventBus = Stores.EventBus

            // Common handler to invalidate cache and reload groups
            const handleGroupChange = () => {
              set(state => {
                state.groupsInitialized = false
              })
              get().loadAllGroups()
            }

            // Type-safe - TypeScript infers event types automatically
            eventBus.on('group.created', handleGroupChange)
            eventBus.on('group.updated', handleGroupChange)
            eventBus.on('group.deleted', handleGroupChange)

            // When groups are assigned to a server, update the cache directly
            eventBus.on('mcp_server.groups_changed', async event => {
              const { serverId, groupIds } = event.data

              // Ensure groups are loaded
              await get().loadAllGroups()

              // Use cached groups to build the assigned list
              const allGroups = get().allGroups
              const assignedGroups = allGroups.filter(g => groupIds.includes(g.id))

              set(state => {
                state.serverGroups.set(serverId, {
                  serverId,
                  groups: assignedGroups,
                  loading: false,
                  error: null,
                  lastFetched: Date.now(),
                })
              })
            })
          },

          // Property-specific initialization - runs when allGroups is first accessed
          allGroups: async () => {
            await get().loadAllGroups()
          },
        },

        /**
         * Load all user groups (cached)
         * Only fetches if not already initialized
         */
        loadAllGroups: async (): Promise<void> => {
          const state = get()

          // If already loading, don't start another fetch
          if (state.groupsLoading) {
            return
          }

          // If already initialized, use cached data
          if (state.groupsInitialized && !state.groupsError) {
            return
          }

          set(state => {
            state.groupsLoading = true
            state.groupsError = null
          })

          try {
            const response = await ApiClient.UserGroup.list({ page: 1, per_page: 1000 })

            set(state => {
              state.allGroups = response.groups
              state.groupsLoading = false
              state.groupsError = null
              state.groupsInitialized = true
            })
          } catch (error) {
            console.error('Failed to load user groups:', error)

            set(state => {
              state.groupsLoading = false
              state.groupsError = error instanceof Error ? error.message : 'Failed to load groups'
            })

            throw error
          }
        },

        /**
         * Load groups for a specific server
         * Uses cached groups instead of fetching every time
         */
        loadGroupsForServer: async (serverId: string, force = false): Promise<void> => {
          const state = get()
          const existing = state.serverGroups.get(serverId)

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
            state.serverGroups.set(serverId, {
              serverId,
              groups: existing?.groups || [],
              loading: true,
              error: null,
              lastFetched: existing?.lastFetched || null,
            })
          })

          try {
            // Ensure groups are loaded
            await get().loadAllGroups()

            // Get group IDs for this server
            const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: serverId })

            // Use cached groups instead of fetching
            const allGroups = get().allGroups
            const assignedGroups = allGroups.filter((g: Group) =>
              groupIds.includes(g.id),
            )

            set(state => {
              state.serverGroups.set(serverId, {
                serverId,
                groups: assignedGroups,
                loading: false,
                error: null,
                lastFetched: Date.now(),
              })
            })
          } catch (error) {
            console.error(`Failed to load groups for server ${serverId}:`, error)

            set(state => {
              state.serverGroups.set(serverId, {
                serverId,
                groups: existing?.groups || [],
                loading: false,
                error: error instanceof Error ? error.message : 'Failed to load groups',
                lastFetched: existing?.lastFetched || null,
              })
            })

            throw error
          }
        },

        /**
         * Clear cached data for a specific server
         */
        clearServerGroups: (serverId: string): void => {
          set(state => {
            state.serverGroups.delete(serverId)
          })
        },

        /**
         * Clear all cached group data
         */
        clearAllServerGroups: (): void => {
          set(state => {
            state.serverGroups.clear()
          })
        },

        /**
         * Get groups for a specific server from the store
         */
        getServerGroupsData: (serverId: string): ServerGroups | undefined => {
          return get().serverGroups.get(serverId)
        },
      }),
    ),
  ),
)
