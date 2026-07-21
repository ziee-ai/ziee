import type { UserGroupsSet, UserGroupsGet } from '../state'

export default (set: UserGroupsSet, _get: UserGroupsGet) => async () => {
  set({ error: null })
}
