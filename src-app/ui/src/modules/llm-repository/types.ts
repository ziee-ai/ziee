import type { StoreProxy } from '@/core/stores'
import type { useLlmRepositoryStore } from './store'
import type { useLlmRepositoryDrawerStore } from './drawer-store'

// Augment the RegisteredStores interface to add LlmRepository stores
declare module '@/core/stores' {
  interface RegisteredStores {
    LlmRepository: StoreProxy<ReturnType<typeof useLlmRepositoryStore.getState>>
    LlmRepositoryDrawer: StoreProxy<ReturnType<typeof useLlmRepositoryDrawerStore.getState>>
  }
}

export {}
