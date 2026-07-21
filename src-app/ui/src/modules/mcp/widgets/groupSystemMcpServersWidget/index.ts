import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  groupSystemMcpServersWidgetState,
  type GroupSystemMcpServersWidgetState,
} from './state'
import type { Actions } from './actions.gen'

const GroupSystemMcpServersWidgetDef = defineStore<GroupSystemMcpServersWidgetState, Actions>(
  'GroupSystemMcpServersWidget',
  {
    immer: true,
    state: groupSystemMcpServersWidgetState,
    actions: import.meta.glob('./actions/*.ts'),
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
  },
)

export const GroupSystemMcpServersWidget = registerLazyStore(GroupSystemMcpServersWidgetDef)
export const useGroupSystemMcpServersWidgetStore = GroupSystemMcpServersWidgetDef.store
export { GroupSystemMcpServersWidgetDef }
