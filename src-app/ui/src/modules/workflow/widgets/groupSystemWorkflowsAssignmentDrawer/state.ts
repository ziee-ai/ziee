import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const groupSystemWorkflowsAssignmentState = {
  isOpen: false,
  selectedGroup: null as Group | null,
}

export type GroupSystemWorkflowsAssignmentDrawerState = typeof groupSystemWorkflowsAssignmentState
export type GroupSystemWorkflowsAssignmentSet = StoreSet<GroupSystemWorkflowsAssignmentDrawerState>
export type GroupSystemWorkflowsAssignmentGet = () => GroupSystemWorkflowsAssignmentDrawerState
