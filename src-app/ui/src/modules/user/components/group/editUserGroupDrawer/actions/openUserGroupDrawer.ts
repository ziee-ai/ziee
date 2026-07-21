import type { Group } from '@/api-client/types'
import type { EditUserGroupDrawerGet, EditUserGroupDrawerSet } from '../state'

export default (set: EditUserGroupDrawerSet, _get: EditUserGroupDrawerGet) =>
  async (group: Group) => {
    set({ isOpen: true, editingGroup: group })
  }
