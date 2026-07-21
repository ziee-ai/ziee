import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { INITIAL, systemMcpServerState, type SystemMcpServerState } from './state'
import type { Actions } from './actions.gen'

/**
 * Admin (deployment-shared) System MCP servers store — folder-glob pattern
 * (`state.ts` + `actions/*.ts` + this index). Uses the EAGER glob form
 * (`{ eager: true }`): its `getSystemServerById()` / `getEnabledSystemServers()`
 * / `searchSystemServers()` / `isServerOperationLoading()` selectors return
 * values consumed SYNCHRONOUSLY in render, so the actions load eagerly rather
 * than behind a deferred dynamic import. Actions auto-register from
 * `./actions/*.ts` by filename.
 */
const SystemMcpServerDef = defineStore<SystemMcpServerState, Actions>('SystemMcpServer', {
  state: systemMcpServerState,
  actions: import.meta.glob('./actions/*.ts', { eager: true }),
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

export const SystemMcpServer = registerLazyStore(SystemMcpServerDef)
export const useSystemMcpServersStore = SystemMcpServerDef.store
