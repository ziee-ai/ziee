import type { StoreProxy } from '@/core/stores'
import type { useLlmRepositoryStore } from '@/modules/llm-repository/stores/LlmRepository.store'
import type { useLlmRepositoryDrawerStore } from '@/modules/llm-repository/components/LlmRepositoryDrawer.store'

// Augment the RegisteredStores interface to add LlmRepository stores
declare module '@/core/stores' {
  interface RegisteredStores {
    LlmRepository: StoreProxy<ReturnType<typeof useLlmRepositoryStore.getState>>
    LlmRepositoryDrawer: StoreProxy<
      ReturnType<typeof useLlmRepositoryDrawerStore.getState>
    >
  }
}

export {}
