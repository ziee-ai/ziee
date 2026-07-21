import type { StoreSet } from '@ziee/framework/store-kit'
import type { KnowledgeBase } from '@/api-client/types'

export const knowledgeBasesState = {
  items: new Map<string, KnowledgeBase>(),
  isInitialized: false,
  loading: false,
  creating: false,
  deleting: false,
  error: null as string | null,
}

export type KnowledgeBasesState = typeof knowledgeBasesState
export type KnowledgeBasesSet = StoreSet<KnowledgeBasesState>
export type KnowledgeBasesGet = () => KnowledgeBasesState
