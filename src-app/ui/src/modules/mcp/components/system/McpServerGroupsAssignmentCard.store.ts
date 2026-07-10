import { Permissions, type Group } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

interface ServerGroups {
  serverId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export const SystemMcpServerGroupCard = defineStore('SystemMcpServerGroupCard', {
  immer: true,
  state: {
    // Map of serverId -> group data
    serverGroups: new Map<string, ServerGroups>(),
    // Cached user groups
    allGroups: [] as Group[],
    groupsLoading: false,
    groupsError: null as string | null,
    groupsInitialized: false,
  },
  actions: (set, get) => {
    // Load all user groups (cached). Only fetches if not already initialized.
    const loadAllGroups = async (): Promise<void> => {
      const state = get()
      if (state.groupsLoading) return
      if (state.groupsInitialized && !state.groupsError) return
      set(s => {
        s.groupsLoading = true
        s.groupsError = null
      })
      try {
        const response = await ApiClient.UserGroup.list({ page: 1, per_page: 1000 })
        set(s => {
          // Defensive: never assign a non-array (downstream reads `.length`/maps).
          s.allGroups = Array.isArray(response.groups) ? response.groups : []
          s.groupsLoading = false
          s.groupsError = null
          s.groupsInitialized = true
        })
      } catch (error) {
        console.error('Failed to load user groups:', error)
        set(s => {
          s.groupsLoading = false
          s.groupsError = error instanceof Error ? error.message : 'Failed to load groups'
        })
        throw error
      }
    }
    return {
      loadAllGroups,
      // Load groups for a specific server; uses cached user groups.
      loadGroupsForServer: async (serverId: string, force = false): Promise<void> => {
        const state = get()
        const existing = state.serverGroups.get(serverId)
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
          s.serverGroups.set(serverId, {
            serverId,
            groups: existing?.groups || [],
            loading: true,
            error: null,
            lastFetched: existing?.lastFetched || null,
          })
        })
        try {
          await loadAllGroups()
          const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: serverId })
          const assignedGroups = get().allGroups.filter((g: Group) => groupIds.includes(g.id))
          set(s => {
            s.serverGroups.set(serverId, {
              serverId,
              groups: assignedGroups,
              loading: false,
              error: null,
              lastFetched: Date.now(),
            })
          })
        } catch (error) {
          console.error(`Failed to load groups for server ${serverId}:`, error)
          set(s => {
            s.serverGroups.set(serverId, {
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
      clearServerGroups: (serverId: string): void => {
        set(s => {
          s.serverGroups.delete(serverId)
        })
      },
      clearAllServerGroups: (): void => {
        set(s => {
          s.serverGroups.clear()
        })
      },
      getServerGroupsData: (serverId: string): ServerGroups | undefined =>
        get().serverGroups.get(serverId),
    }
  },
  init: ({ on, get, set, actions }) => {
    const handleGroupChange = () => {
      set(s => {
        s.groupsInitialized = false
      })
      void actions.loadAllGroups()
    }
    on('group.created', handleGroupChange)
    on('group.updated', handleGroupChange)
    on('group.deleted', handleGroupChange)
    // When groups are assigned to a server, update the cache directly.
    on('mcp_server.groups_changed', async event => {
      const { serverId, groupIds } = event.data
      await actions.loadAllGroups()
      const assignedGroups = get().allGroups.filter(g => groupIds.includes(g.id))
      set(s => {
        s.serverGroups.set(serverId, {
          serverId,
          groups: assignedGroups,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    })
    // `GET /api/groups` requires groups::read (not user-held). Guard the eager
    // load so a scoped admin without it doesn't 403 at store-mount.
    if (hasPermissionNow(Permissions.GroupsRead)) {
      void actions.loadAllGroups()
    }
  },
})

export const useSystemMcpServerGroupCardStore = SystemMcpServerGroupCard.store
