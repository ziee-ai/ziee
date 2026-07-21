import type { User } from '@/api-client/types'
import type { ResetPasswordDrawerGet, ResetPasswordDrawerSet } from '../state'

export default (set: ResetPasswordDrawerSet, _get: ResetPasswordDrawerGet) =>
  async (user: User) => {
    set({ isOpen: true, user })
  }
