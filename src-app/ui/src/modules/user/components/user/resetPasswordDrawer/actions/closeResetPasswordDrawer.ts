import type { ResetPasswordDrawerGet, ResetPasswordDrawerSet } from '../state'

export default (set: ResetPasswordDrawerSet, _get: ResetPasswordDrawerGet) =>
  async () => {
    set({ isOpen: false, user: null })
  }
