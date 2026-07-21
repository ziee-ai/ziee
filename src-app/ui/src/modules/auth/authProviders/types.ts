import type { StoreProxy } from '@ziee/framework/stores'
import type { useAuthProvidersStore } from './index'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AuthProviders: StoreProxy<ReturnType<typeof useAuthProvidersStore.getState>>
  }
}

export {}
