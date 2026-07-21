import type { StoreProxy } from '@ziee/framework/stores'
import type { useServerUpdateStore } from './stores/serverUpdate'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    ServerUpdate: StoreProxy<ReturnType<typeof useServerUpdateStore.getState>>
  }
}

export {}
