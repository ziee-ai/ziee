import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const groupSystemMcpServersAssignmentDrawerState = {
  isOpen: false,
  selectedGroup: null as Group | null,
}

export type GroupSystemMcpServersAssignmentDrawerState = typeof groupSystemMcpServersAssignmentDrawerState
export type GroupSystemMcpServersAssignmentDrawerSet = StoreSet<GroupSystemMcpServersAssignmentDrawerState>
export type GroupSystemMcpServersAssignmentDrawerGet = () => GroupSystemMcpServersAssignmentDrawerState
