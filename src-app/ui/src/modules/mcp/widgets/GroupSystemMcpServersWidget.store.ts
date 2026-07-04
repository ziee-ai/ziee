import type { McpServer } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { defineStore } from '@/core/store-kit'

interface GroupServers {
  groupId: string
  servers: McpServer[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export const GroupSystemMcpServersWidget = defineStore('GroupSystemMcpServersWidget', {
  immer: true,
  state: {
    // Map of groupId -> server data
    groupServers: new Map<string, GroupServers>(),
    // Cached servers
    allServers: [] as McpServer[],
    serversLoading: false,
    serversError: null as string | null,
    serversInitialized: false,
  },
  actions: (set, get) => {
    // Load all system servers (cached). Only fetches if not already initialized.
    const loadAllServers = async (): Promise<void> => {
      const state = get()
      if (state.serversLoading) return
      if (state.serversInitialized && !state.serversError) return
      set(s => {
        s.serversLoading = true
        s.serversError = null
      })
      try {
        const response = await ApiClient.McpServerSystem.list({ page: 1, per_page: 1000 })
        set(s => {
          s.allServers = response.servers
          s.serversLoading = false
          s.serversError = null
          s.serversInitialized = true
        })
      } catch (error) {
        console.error('Failed to load servers:', error)
        set(s => {
          s.serversLoading = false
          s.serversError = error instanceof Error ? error.message : 'Failed to load servers'
        })
        throw error
      }
    }
    return {
      loadAllServers,
      // Load servers for a specific group; uses cached servers.
      loadServersForGroup: async (groupId: string, force = false): Promise<void> => {
        const state = get()
        const existing = state.groupServers.get(groupId)
        if (existing?.loading && !force) return
        if (
          !force &&
          existing?.lastFetched &&
          Date.now() - existing.lastFetched < 30000 &&
          !existing.error
        ) {
          return
        }
        set(s => {
          s.groupServers.set(groupId, {
            groupId,
            servers: existing?.servers || [],
            loading: true,
            error: null,
            lastFetched: existing?.lastFetched || null,
          })
        })
        try {
          await loadAllServers()
          const allServers = get().allServers
          // For each server, check if it's assigned to this group.
          const assignedServers: McpServer[] = []
          for (const server of allServers) {
            const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: server.id })
            if (groupIds.includes(groupId)) assignedServers.push(server)
          }
          set(s => {
            s.groupServers.set(groupId, {
              groupId,
              servers: assignedServers,
              loading: false,
              error: null,
              lastFetched: Date.now(),
            })
          })
        } catch (error) {
          console.error(`Failed to load servers for group ${groupId}:`, error)
          set(s => {
            s.groupServers.set(groupId, {
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
      clearGroupServers: (groupId: string): void => {
        set(s => {
          s.groupServers.delete(groupId)
        })
      },
      clearAllGroupServers: (): void => {
        set(s => {
          s.groupServers.clear()
        })
      },
      getGroupServersData: (groupId: string): GroupServers | undefined =>
        get().groupServers.get(groupId),
    }
  },
  init: ({ on, get, set, actions }) => {
    // When servers are assigned to a group, update the cache directly.
    on('mcp_server.group_servers_changed', async event => {
      const { groupId, serverIds } = event.data
      await actions.loadAllServers()
      const assignedServers = get().allServers.filter(s => serverIds.includes(s.id))
      set(s => {
        s.groupServers.set(groupId, {
          groupId,
          servers: assignedServers,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    })
    on('mcp_server.created', async event => {
      // Only handle system servers.
      if (event.data.server.is_system) {
        set(s => {
          s.serversInitialized = false
        })
        await actions.loadAllServers()
      }
    })
    on('mcp_server.updated', event => {
      const { server } = event.data
      if (server.is_system) {
        set(s => {
          const index = s.allServers.findIndex(x => x.id === server.id)
          if (index !== -1) s.allServers[index] = server
        })
      }
    })
    on('mcp_server.deleted', event => {
      const { serverId } = event.data
      set(s => {
        s.allServers = s.allServers.filter(x => x.id !== serverId)
        s.groupServers.forEach((groupData, groupId) => {
          s.groupServers.set(groupId, {
            ...groupData,
            servers: groupData.servers.filter(x => x.id !== serverId),
          })
        })
      })
    })
  },
})

export const useGroupSystemMcpServersWidgetStore = GroupSystemMcpServersWidget.store
