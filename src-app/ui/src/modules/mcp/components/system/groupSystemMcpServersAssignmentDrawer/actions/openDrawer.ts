import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { Group } from '@/api-client/types'
import type { GroupSystemMcpServersAssignmentDrawerGet, GroupSystemMcpServersAssignmentDrawerSet } from '../state'

export default (set: GroupSystemMcpServersAssignmentDrawerSet, _get: GroupSystemMcpServersAssignmentDrawerGet) => {
  return async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
    setOverlayOpen('group-mcp-assignment', true)
  }
}
