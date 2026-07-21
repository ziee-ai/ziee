import type { UserMemory } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const memoriesState = {
  memories: [] as UserMemory[],
  loading: false,
  saving: false,
  error: null as string | null,
  searchQuery: '',
  kindFilter: null as string | null,
  sourceFilter: null as string | null,
  // Pagination state — drives MyMemoriesSection's <Pagination>.
  currentPage: 1,
  pageSize: 10,
  total: 0,
}

export type MemoriesState = typeof memoriesState
export type MemoriesSet = StoreSet<MemoriesState>
export type MemoriesGet = () => MemoriesState
