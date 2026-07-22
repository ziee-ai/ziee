import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { GroupSystemMcpServersAssignmentDrawerGet, GroupSystemMcpServersAssignmentDrawerSet } from '../state'

export default (set: GroupSystemMcpServersAssignmentDrawerSet, _get: GroupSystemMcpServersAssignmentDrawerGet) => {
  return async () => {
    set({ isOpen: false, selectedGroup: null })
    setOverlayOpen('group-mcp-assignment', false)
  }
}
