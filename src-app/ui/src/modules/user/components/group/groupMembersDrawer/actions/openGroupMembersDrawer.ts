import type { Group } from '@/api-client/types'
import type { GroupMembersDrawerSet } from '../state'

export default (set: GroupMembersDrawerSet) =>
  async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
  }
