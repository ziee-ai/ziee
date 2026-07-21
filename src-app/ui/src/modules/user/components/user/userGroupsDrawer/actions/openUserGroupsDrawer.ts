import type { User } from '@/api-client/types'
import type { UserGroupsDrawerGet, UserGroupsDrawerSet } from '../state'

export default (set: UserGroupsDrawerSet, _get: UserGroupsDrawerGet) =>
  async (user: User) => {
    set({ isOpen: true, user })
  }
