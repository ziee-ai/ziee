import type { StoreProxy } from '@/core/stores'
import type { useMcpStore, useSystemMcpServersStore, useMcpServerDrawerStore } from './stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    McpServer: StoreProxy<ReturnType<typeof useMcpStore.getState>>
    SystemMcpServer: StoreProxy<ReturnType<typeof useSystemMcpServersStore.getState>>
    McpServerDrawer: StoreProxy<ReturnType<typeof useMcpServerDrawerStore.getState>>
  }
}

export {}
