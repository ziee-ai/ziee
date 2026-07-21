import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export interface GroupMember {
  id: string
  username: string
  email: string
  is_active: boolean
  joined_at: string
}

export const userGroupsState = {
  groups: [] as Group[],
  currentGroupMembers: [] as GroupMember[],
  total: 0,
  currentPage: 1,
  pageSize: 10,
  isInitialized: false,
  currentGroupId: null as string | null,
  loading: false,
  loadingGroups: false,
  loadingGroupMembers: false,
  creating: false,
  updating: false,
  deleting: false,
  error: null as string | null,
}

export type UserGroupsState = typeof userGroupsState
export type UserGroupsSet = StoreSet<UserGroupsState>
export type UserGroupsGet = () => UserGroupsState

// Tracks in-flight group-member fetch promises so concurrent requests for the
// same group await the in-flight call instead of returning stale data (a false
// negative when a user belongs to 2+ groups).
export const pendingMemberRequests = new Map<string, Promise<void>>()
