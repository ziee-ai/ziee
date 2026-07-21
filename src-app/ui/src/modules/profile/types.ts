import type { StoreProxy } from '@ziee/framework/stores'
import type { useProfileStore } from './stores/profile'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Profile: StoreProxy<ReturnType<typeof useProfileStore.getState>>
  }
}

export {}
