import type { StoreProxy } from '@/core/stores'
import type { useMcpStore, useSystemMcpServersStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    McpServer: StoreProxy<ReturnType<typeof useMcpStore.getState>>
    SystemMcpServer: StoreProxy<ReturnType<typeof useSystemMcpServersStore.getState>>
  }
}

export {}
