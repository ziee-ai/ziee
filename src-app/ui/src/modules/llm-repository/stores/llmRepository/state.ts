import type { StoreSet } from '@ziee/framework/store-kit'
import type { LlmRepository } from '@/api-client/types'

export const llmRepositoryState = {
  repositories: [] as LlmRepository[],
  isInitialized: false,
  // Pagination defaults match the settings page's pageSizeOptions.
  currentPage: 1,
  pageSize: 10,
  total: 0,
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  testing: false,
  error: null as string | null,
}

export type LlmRepositoryState = typeof llmRepositoryState
export type LlmRepositorySet = StoreSet<LlmRepositoryState>
export type LlmRepositoryGet = () => LlmRepositoryState
