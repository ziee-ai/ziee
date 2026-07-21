import type { GroupSystemMcpServersAssignmentDrawerGet, GroupSystemMcpServersAssignmentDrawerSet } from '../state'

export default (set: GroupSystemMcpServersAssignmentDrawerSet, _get: GroupSystemMcpServersAssignmentDrawerGet) => {
  return async () => {
    set({ isOpen: false, selectedGroup: null })
  }
}
