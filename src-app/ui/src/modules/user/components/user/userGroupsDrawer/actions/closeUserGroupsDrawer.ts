import type { UserGroupsDrawerGet, UserGroupsDrawerSet } from '../state'

export default (set: UserGroupsDrawerSet, _get: UserGroupsDrawerGet) =>
  async () => {
    set({ isOpen: false, user: null })
  }
