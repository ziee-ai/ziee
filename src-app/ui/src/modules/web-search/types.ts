import type { StoreProxy } from '@/core/stores'
import type { useWebSearchAdminStore } from './stores/WebSearchAdmin.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    WebSearchAdmin: StoreProxy<
      ReturnType<typeof useWebSearchAdminStore.getState>
    >
  }
}

export {}
