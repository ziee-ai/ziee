import type { StoreProxy } from '@ziee/framework/stores'
import type { useWebSearchAdminStore } from './stores/WebSearchAdmin.store'
import type { useWebSearchUserKeysStore } from './stores/WebSearchUserKeys.store'

declare module '@ziee/framework/stores' {
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
