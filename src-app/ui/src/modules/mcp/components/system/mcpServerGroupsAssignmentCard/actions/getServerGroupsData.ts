import type { McpServerGroupsAssignmentCardGet, McpServerGroupsAssignmentCardSet, ServerGroups } from '../state'

export default (_set: McpServerGroupsAssignmentCardSet, get: McpServerGroupsAssignmentCardGet) =>
  (serverId: string): ServerGroups | undefined =>
    get().serverGroups.get(serverId)
