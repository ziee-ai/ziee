import type { McpServerGroupsAssignmentCardGet, McpServerGroupsAssignmentCardSet } from '../state'

export default (set: McpServerGroupsAssignmentCardSet, _get: McpServerGroupsAssignmentCardGet) =>
  (serverId: string): void => {
    set(s => {
      s.serverGroups.delete(serverId)
    })
  }
