import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { mcpServerGroupsAssignmentCardState, type McpServerGroupsAssignmentCardState } from './state'
import type { Actions } from './actions.gen'

const McpServerGroupsAssignmentCardDef = defineStore<McpServerGroupsAssignmentCardState, Actions>('SystemMcpServerGroupCard', {
  immer: true,
  state: mcpServerGroupsAssignmentCardState,
  actions: import.meta.glob('./actions/*.ts'),
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
export const McpServerGroupsAssignmentCard = registerLazyStore(McpServerGroupsAssignmentCardDef)
export const useSystemMcpServerGroupCardStore = McpServerGroupsAssignmentCardDef.store
export const SystemMcpServerGroupCard = McpServerGroupsAssignmentCard
