import type { StoreProxy } from '@/core/stores'
import type { useLlmRepositoryStore } from './store'

// Augment the RegisteredStores interface to add LlmRepository store
declare module '@/core/stores' {
  interface RegisteredStores {
    LlmRepository: StoreProxy<ReturnType<typeof useLlmRepositoryStore.getState>>
  }
}

export {}
