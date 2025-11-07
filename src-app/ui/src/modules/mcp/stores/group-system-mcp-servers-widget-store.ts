import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { McpServer } from '@/api-client/types'
import { ApiClient } from '@/api-client'

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

  // Initialization
  __init__: Record<string, never>

  // Actions (methods within store)
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
        __init__: {},

        /**
         * Load servers for a specific group
         * Caches results and prevents duplicate fetches
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
            // Get all system servers
            const response = await ApiClient.McpServerSystem.list({ page: 1, per_page: 1000 })

            // For each server, check if it's assigned to this group
            const assignedServers: McpServer[] = []
            for (const server of response.servers) {
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
