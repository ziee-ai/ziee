import { type Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const providerGroupAssignmentCardState = {
  // Map of providerId -> group data
  providerGroups: new Map<string, ProviderGroups>(),
  // Cached user groups
  allGroups: [] as Group[],
  groupsLoading: false,
  groupsError: null as string | null,
  groupsInitialized: false,
}

export type ProviderGroups = {
  providerId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export type ProviderGroupAssignmentCardState = typeof providerGroupAssignmentCardState
export type ProviderGroupAssignmentCardSet = StoreSet<ProviderGroupAssignmentCardState>
export type ProviderGroupAssignmentCardGet = () => ProviderGroupAssignmentCardState
