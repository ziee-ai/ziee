import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { Group } from '@/api-client/types'
import type { GroupSystemWorkflowsAssignmentGet, GroupSystemWorkflowsAssignmentSet } from '../state'

export default (set: GroupSystemWorkflowsAssignmentSet, _get: GroupSystemWorkflowsAssignmentGet) =>
  async (group: Group) => {
    set({ isOpen: true, selectedGroup: group })
    setOverlayOpen('group-workflow-assignment', true)
  }
