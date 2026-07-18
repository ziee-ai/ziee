import type { StoreProxy } from '@ziee/framework/stores'
import type { useConversationSummarizationStore } from './stores/ConversationSummarization.store'
import type { useSummarizationAdminStore } from './stores/SummarizationAdmin.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    SummarizationAdmin: StoreProxy<
      ReturnType<typeof useSummarizationAdminStore.getState>
    >
    ConversationSummarization: StoreProxy<
      ReturnType<typeof useConversationSummarizationStore.getState>
    >
  }
}

export {}
