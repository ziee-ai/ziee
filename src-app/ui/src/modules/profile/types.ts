import type { StoreProxy } from '@/core/stores'
import type { useProfileStore } from './stores/Profile.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Profile: StoreProxy<ReturnType<typeof useProfileStore.getState>>
  }
}

export {}
