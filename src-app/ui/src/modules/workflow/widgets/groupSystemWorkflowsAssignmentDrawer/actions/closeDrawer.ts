import type { GroupSystemWorkflowsAssignmentGet, GroupSystemWorkflowsAssignmentSet } from '../state'

export default (set: GroupSystemWorkflowsAssignmentSet, _get: GroupSystemWorkflowsAssignmentGet) =>
  async () => {
    set({ isOpen: false, selectedGroup: null })
  }
