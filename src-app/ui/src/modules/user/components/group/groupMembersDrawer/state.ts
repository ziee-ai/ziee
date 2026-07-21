import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const groupMembersDrawerState = {
  isOpen: false,
  selectedGroup: null as Group | null,
}

export type GroupMembersDrawerState = typeof groupMembersDrawerState
export type GroupMembersDrawerSet = StoreSet<GroupMembersDrawerState>
export type GroupMembersDrawerGet = () => GroupMembersDrawerState
