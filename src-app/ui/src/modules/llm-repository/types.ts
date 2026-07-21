import type { StoreProxy } from '@ziee/framework/stores'
import type { useLlmRepositoryStore } from '@/modules/llm-repository/stores/llmRepository'
import type { useLlmRepositoryDrawerStore } from '@/modules/llm-repository/stores/llmRepositoryDrawer'

// Augment the RegisteredStores interface to add LlmRepository stores
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    LlmRepository: StoreProxy<ReturnType<typeof useLlmRepositoryStore.getState>>
    LlmRepositoryDrawer: StoreProxy<
      ReturnType<typeof useLlmRepositoryDrawerStore.getState>
    >
  }
}

export {}
