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
})
