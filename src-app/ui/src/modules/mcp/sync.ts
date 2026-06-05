import { registerSync } from '@/core/sync'
import { useMcpStore } from '@/modules/mcp/stores/McpServer.store'

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
