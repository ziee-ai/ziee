import type { StoreProxy } from '@/core/stores'
import type { useChatLlmProviderStore } from './stores/LlmProvider.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    ChatLlmProvider: StoreProxy<ReturnType<typeof useChatLlmProviderStore.getState>>
  }
}

export {}
