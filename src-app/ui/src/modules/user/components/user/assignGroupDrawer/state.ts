import type { User } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const assignGroupDrawerState = {
  isOpen: false,
  user: null as User | null,
}

export type AssignGroupDrawerState = typeof assignGroupDrawerState
export type AssignGroupDrawerSet = StoreSet<AssignGroupDrawerState>
export type AssignGroupDrawerGet = () => AssignGroupDrawerState
