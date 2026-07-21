import type { EditUserGroupDrawerSet } from '../state'

export default (set: EditUserGroupDrawerSet, _get: () => unknown) =>
  async () => {
    set({ isOpen: false, editingGroup: null })
  }
