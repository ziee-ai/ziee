import type { McpServerGroupsAssignmentCardGet, McpServerGroupsAssignmentCardSet } from '../state'

export default (set: McpServerGroupsAssignmentCardSet, _get: McpServerGroupsAssignmentCardGet) =>
  (): void => {
    set(s => {
      s.serverGroups.clear()
    })
  }
