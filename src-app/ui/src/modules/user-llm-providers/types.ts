import type { StoreProxy } from '@/core/stores'
import type { useUserLlmProvidersStore } from './UserLlmProviders.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserLlmProviders: StoreProxy<ReturnType<typeof useUserLlmProvidersStore.getState>>
  }
}

export {}
