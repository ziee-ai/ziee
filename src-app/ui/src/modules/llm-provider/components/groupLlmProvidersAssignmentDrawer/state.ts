import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const groupLlmProvidersAssignmentState = {
  isOpen: false,
  selectedGroup: null as Group | null,
}

export type GroupLlmProvidersAssignmentDrawerState = typeof groupLlmProvidersAssignmentState
export type GroupLlmProvidersAssignmentDrawerSet = StoreSet<GroupLlmProvidersAssignmentDrawerState>
export type GroupLlmProvidersAssignmentDrawerGet = () => GroupLlmProvidersAssignmentDrawerState
