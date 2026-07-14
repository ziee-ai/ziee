import { useEffect } from 'react'
import { ApiClient } from '@/api-client'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { UserGroupAssignment } from '@/components/common/UserGroupAssignment'
import { emitMcpServerGroupsChanged } from '@/modules/mcp/events'

interface McpServerGroupsAssignmentCardProps {
  serverId: string
}

/**
 * Section for managing which user groups have access to a system MCP server.
 * Thin wrapper over the shared UserGroupAssignment; Assign opens the shared
 * editor Drawer, and save persists via SystemMcpServer.assignServerToGroups.
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
        editor={{
          loadAllGroups: async () => {
            const res = await ApiClient.UserGroup.list({ page: 1, per_page: 100 })
            return res.groups.map(g => ({ id: g.id, name: g.name, description: g.description, is_default: g.is_default }))
          },
          save: async ids => {
            await Stores.SystemMcpServer.assignServerToGroups(serverId, ids)
            await emitMcpServerGroupsChanged(serverId, ids)
          },
        }}
      />
    </div>
  )
}
