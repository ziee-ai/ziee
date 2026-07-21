import type { User } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const editUserDrawerState = {
  isOpen: false,
  editingUser: null as User | null,
}

export type EditUserDrawerState = typeof editUserDrawerState
export type EditUserDrawerSet = StoreSet<EditUserDrawerState>
export type EditUserDrawerGet = () => EditUserDrawerState
