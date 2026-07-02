import type { StoreProxy } from '@/core/stores'
import type { useWebSearchAdminStore } from './stores/WebSearchAdmin.store'
import type { useWebSearchUserKeysStore } from './stores/WebSearchUserKeys.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    WebSearchAdmin: StoreProxy<
      ReturnType<typeof useWebSearchAdminStore.getState>
    >
    WebSearchUserKeys: StoreProxy<
      ReturnType<typeof useWebSearchUserKeysStore.getState>
    >
  }
}

export {}
