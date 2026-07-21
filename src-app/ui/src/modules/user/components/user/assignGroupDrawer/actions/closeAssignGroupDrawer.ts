import type { AssignGroupDrawerGet, AssignGroupDrawerSet } from '../state'

export default (set: AssignGroupDrawerSet, _get: AssignGroupDrawerGet) =>
  async () => {
    set({ isOpen: false, user: null })
  }
