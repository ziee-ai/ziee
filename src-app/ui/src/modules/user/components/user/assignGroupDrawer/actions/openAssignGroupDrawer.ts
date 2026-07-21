import type { User } from '@/api-client/types'
import type { AssignGroupDrawerGet, AssignGroupDrawerSet } from '../state'

export default (set: AssignGroupDrawerSet, _get: AssignGroupDrawerGet) =>
  async (user: User) => {
    set({ isOpen: true, user })
  }
