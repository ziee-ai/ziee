import type { GroupLlmProvidersAssignmentDrawerGet, GroupLlmProvidersAssignmentDrawerSet } from '../state'

export default (set: GroupLlmProvidersAssignmentDrawerSet, _get: GroupLlmProvidersAssignmentDrawerGet) => {
  return async () => {
    set({ isOpen: false, selectedGroup: null })
  }
}
