import type { StoreSet } from '@ziee/framework/store-kit'
import type { Assistant } from '@/api-client/types'

export const userAssistantsState = {
  assistants: [] as Assistant[],
  total: 0,
  currentPage: 1,
  pageSize: 10,
  isInitialized: false,
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  error: null as string | null,
}

export type UserAssistantsState = typeof userAssistantsState
export type UserAssistantsSet = StoreSet<UserAssistantsState>
export type UserAssistantsGet = () => UserAssistantsState
