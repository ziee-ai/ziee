import type { StoreProxy } from '@/core/stores'
import type { useRoutesStore } from '@/modules/router/stores/routes-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Routes: StoreProxy<ReturnType<typeof useRoutesStore.getState>>
  }
}

export {}
