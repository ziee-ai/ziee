import type { StoreProxy } from '@/core/stores'
import type { useLlmProviderStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    LlmProvider: StoreProxy<ReturnType<typeof useLlmProviderStore.getState>>
  }
}

export {}
