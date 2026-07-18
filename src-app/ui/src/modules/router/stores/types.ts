import type { StoreProxy } from '@ziee/framework/stores'
import type { useRoutesStore } from '@/modules/router/stores/routes-store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Routes: StoreProxy<ReturnType<typeof useRoutesStore.getState>>
  }
}

export {}
