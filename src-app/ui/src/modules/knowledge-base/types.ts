import type { StoreProxy } from '@/core/stores'
import type {
  useKnowledgeBaseComposerStore,
  useKnowledgeBaseDetailStore,
  useKnowledgeBasesStore,
} from '@/modules/knowledge-base/stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    KnowledgeBases: StoreProxy<ReturnType<typeof useKnowledgeBasesStore.getState>>
    KnowledgeBaseDetail: StoreProxy<
      ReturnType<typeof useKnowledgeBaseDetailStore.getState>
    >
    KnowledgeBaseComposer: StoreProxy<
      ReturnType<typeof useKnowledgeBaseComposerStore.getState>
    >
  }
}

export {}
