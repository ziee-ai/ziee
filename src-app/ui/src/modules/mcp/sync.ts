import { Permissions } from '@/api-client/types'
import { registerSync } from '@/core/sync'
import { useMcpStore } from '@/modules/mcp/stores/McpServer.store'
import { useSystemMcpServersStore } from '@/modules/mcp/stores/SystemMcpServer.store'

// User-owned MCP servers. `loadMcpServers()` reloads the current page
// (permission-gated internally), surfacing remote create/update/delete.
registerSync('mcp_server', {
  onEvent: () => {
    void useMcpStore.getState().loadMcpServers()
  },
  onResync: () => {
    void useMcpStore.getState().loadMcpServers()
  },
})

// Admin system (deployment-shared) MCP servers table.
registerSync('mcp_server_system', {
  onEvent: () => {
    void useSystemMcpServersStore.getState().loadSystemServers()
  },
  onResync: () => {
    void useSystemMcpServersStore.getState().loadSystemServers()
  },
  requiredPermission: Permissions.McpServersAdminRead,
})

// A system server's group-visibility changed → a user's ACCESSIBLE server
// set may have changed. `loadMcpServers` fetches the accessible set
// (personal + system-via-groups) via listAccessible, so reloading it
// refreshes the user's view; each client only sees its own scoped set.
registerSync('user_mcp_server', {
  onEvent: () => {
    void useMcpStore.getState().loadMcpServers()
  },
  onResync: () => {
    void useMcpStore.getState().loadMcpServers()
  },
})
