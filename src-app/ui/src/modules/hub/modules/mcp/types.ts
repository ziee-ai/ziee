import type { StoreProxy } from '@/core/stores'
import type { useHubMcpServersStore } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'
import type { useMcpServerDetailsDrawerStore } from '@/modules/hub/modules/mcp/components/McpServerDetailsDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubMcpServers: StoreProxy<ReturnType<typeof useHubMcpServersStore.getState>>
    McpServerDetailsDrawer: StoreProxy<
      ReturnType<typeof useMcpServerDetailsDrawerStore.getState>
    >
  }
}

export {}
