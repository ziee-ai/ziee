import type { StoreProxy } from '@/core/stores'
import type { useRouterStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Router: StoreProxy<ReturnType<typeof useRouterStore.getState>>
  }
}

export {}
