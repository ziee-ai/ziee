import type { GroupMembersDrawerSet } from '../state'

export default (set: GroupMembersDrawerSet) => async () => {
  set({ isOpen: false, selectedGroup: null })
}
