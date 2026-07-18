import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useKnowledgeBaseComposerStore,
  useKnowledgeBaseDetailStore,
  useKnowledgeBasesStore,
} from '@/modules/knowledge-base/stores'

declare module '@ziee/framework/stores' {
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
