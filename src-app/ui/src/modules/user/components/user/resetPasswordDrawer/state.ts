import type { User } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const resetPasswordDrawerState = { isOpen: false, user: null as User | null }

export type ResetPasswordDrawerState = typeof resetPasswordDrawerState
export type ResetPasswordDrawerSet = StoreSet<ResetPasswordDrawerState>
export type ResetPasswordDrawerGet = () => ResetPasswordDrawerState
