import type { EditUserDrawerGet, EditUserDrawerSet } from '../state'

export default (set: EditUserDrawerSet, _get: EditUserDrawerGet) => () => {
  set({ isOpen: false, editingUser: null })
}
