import type { StoreProxy } from '@/core/stores'
import type {
  useKnowledgeBaseDetailStore,
  useKnowledgeBasesStore,
} from '@/modules/knowledge-base/stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    KnowledgeBases: StoreProxy<ReturnType<typeof useKnowledgeBasesStore.getState>>
    KnowledgeBaseDetail: StoreProxy<
      ReturnType<typeof useKnowledgeBaseDetailStore.getState>
    >
  }
}

export {}
