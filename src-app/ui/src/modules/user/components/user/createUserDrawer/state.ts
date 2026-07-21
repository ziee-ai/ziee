import type { StoreSet } from '@ziee/framework/store-kit'

export const createUserDrawerState = { isOpen: false }

export type CreateUserDrawerState = typeof createUserDrawerState
export type CreateUserDrawerSet = StoreSet<CreateUserDrawerState>
export type CreateUserDrawerGet = () => CreateUserDrawerState
