import type { StoreProxy } from '@/core/stores'
import type { useLlmProviderStore, useLlmModelDownloadStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    LlmProvider: StoreProxy<ReturnType<typeof useLlmProviderStore.getState>>
    LlmModelDownload: StoreProxy<
      ReturnType<typeof useLlmModelDownloadStore.getState>
    >
  }
}

export {}
