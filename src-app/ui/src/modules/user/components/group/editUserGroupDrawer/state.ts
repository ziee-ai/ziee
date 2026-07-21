import type { Group } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const editUserGroupDrawerState = {
  isOpen: false,
  editingGroup: null as Group | null,
}

export type EditUserGroupDrawerState = typeof editUserGroupDrawerState
export type EditUserGroupDrawerSet = StoreSet<EditUserGroupDrawerState>
export type EditUserGroupDrawerGet = () => EditUserGroupDrawerState
