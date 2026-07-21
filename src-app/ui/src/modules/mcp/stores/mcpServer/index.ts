import { enableMapSet } from 'immer'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { mcpServerState, type McpServerState } from './state'
import type { Actions } from './actions.gen'

enableMapSet()

const McpServerDef = defineStore<McpServerState, Actions>('McpServer', {
  immer: true,
  state: mcpServerState,
  actions: import.meta.glob('./actions/*.ts'),
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

export const McpServer = registerLazyStore(McpServerDef)
export const useMcpStore = McpServerDef.store

// Raw store definition export for compatibility.
export { McpServerDef }
