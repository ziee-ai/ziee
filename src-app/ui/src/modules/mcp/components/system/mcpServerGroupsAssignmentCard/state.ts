import { type Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const mcpServerGroupsAssignmentCardState = {
  // Map of serverId -> group data
  serverGroups: new Map<string, ServerGroups>(),
  // Cached user groups
  allGroups: [] as Group[],
  groupsLoading: false,
  groupsError: null as string | null,
  groupsInitialized: false,
}

export type ServerGroups = {
  serverId: string
  groups: Group[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

export type McpServerGroupsAssignmentCardState = typeof mcpServerGroupsAssignmentCardState
export type McpServerGroupsAssignmentCardSet = StoreSet<McpServerGroupsAssignmentCardState>
export type McpServerGroupsAssignmentCardGet = () => McpServerGroupsAssignmentCardState
