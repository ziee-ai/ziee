import type { StoreSet } from '@ziee/framework/store-kit'
import type { LlmRepository } from '@/api-client/types'

export const llmRepositoryDrawerState = {
  open: false,
  loading: false,
  editingRepository: null as LlmRepository | null,
}

export type LlmRepositoryDrawerState = typeof llmRepositoryDrawerState
export type LlmRepositoryDrawerSet = StoreSet<LlmRepositoryDrawerState>
export type LlmRepositoryDrawerGet = () => LlmRepositoryDrawerState
