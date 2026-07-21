import type { Group } from '@/api-client/types'
import type { GroupSystemMcpServersAssignmentDrawerGet, GroupSystemMcpServersAssignmentDrawerSet } from '../state'

export default (set: GroupSystemMcpServersAssignmentDrawerSet, _get: GroupSystemMcpServersAssignmentDrawerGet) => {
  return async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
  }
}
