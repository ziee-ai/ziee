import type { User } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const userGroupsDrawerState = { isOpen: false, user: null as User | null }

export type UserGroupsDrawerState = typeof userGroupsDrawerState
export type UserGroupsDrawerSet = StoreSet<UserGroupsDrawerState>
export type UserGroupsDrawerGet = () => UserGroupsDrawerState
