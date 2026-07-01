import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { UserGroupAssignment } from '@/components/common/UserGroupAssignment'

interface McpServerGroupsAssignmentCardProps {
  serverId: string
}

/**
 * Section for managing which user groups have access to a system MCP server.
 * Thin wrapper over the shared UserGroupAssignment; editing happens in a
 * dedicated drawer (opened via `onAssign`) rather than an inline editor.
 */
export function McpServerGroupsAssignmentCard({
  serverId,
}: McpServerGroupsAssignmentCardProps) {
  const serverData = Stores.SystemMcpServerGroupCard.serverGroups.get(serverId)
  const assignedGroups = serverData?.groups || []
  const loading = serverData?.loading || false
  const canManage = usePermission(Permissions.McpServersAdminEdit)

  useEffect(() => {
    Stores.SystemMcpServerGroupCard.loadGroupsForServer(serverId)
  }, [serverId])

  return (
    <div data-server-id={serverId} data-card-type="user-groups-assignment">
      <UserGroupAssignment
        data-testid={`mcp-groups-${serverId}`}
        assignedGroups={assignedGroups.map(g => ({ id: g.id, name: g.name }))}
        loading={loading}
        canAssign={canManage}
        emptyText="No groups assigned"
        description="User groups that have access to this MCP server"
        onAssign={() => Stores.McpServerGroupsAssignment.openDrawer(serverId)}
      />
    </div>
  )
}
