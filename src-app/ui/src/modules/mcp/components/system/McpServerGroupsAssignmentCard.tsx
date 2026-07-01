import { useEffect } from 'react'
import { Pencil } from 'lucide-react'
import { Button, Flex, Space, Spin, Tag, Text } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

interface McpServerGroupsAssignmentCardProps {
  serverId: string
}

/**
 * Section for managing which user groups have access to a system MCP server.
 * Displays assigned groups and opens a drawer for management.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 *
 * IMPORTANT: Section fetches data on mount AND listens to events for real-time updates.
 * This ensures data is loaded even after page reloads.
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

  const handleManageGroups = () => {
    Stores.McpServerGroupsAssignment.openDrawer(serverId)
  }

  return (
    // pb-3 keeps the User Groups section from flush-bottoming
    // against the parent McpServerCard's edge — gives the same
    // breathing room as the rest of the card's interior padding.
    <div
      className="pb-3"
      data-server-id={serverId}
      data-card-type="user-groups-assignment"
    >
      {/* Always-visible User Groups section (no accordion): the "User Groups"
          heading with the Assign action next to it, then the list below. */}
      <Flex align="center" className="gap-2 mb-1">
        <Text className="font-medium text-sm">User Groups</Text>
        {canManage && (
          <Button
            variant="ghost"
            size="default"
            icon={<Pencil aria-hidden="true" />}
            onClick={handleManageGroups}
            aria-label="Manage user groups"
            data-testid={`mcp-groups-assign-btn-${serverId}`}
          >
            Assign
          </Button>
        )}
      </Flex>
      {loading ? (
        <Spin size="sm" label="Loading" />
      ) : assignedGroups.length === 0 ? (
        <Text
          type="secondary"
          className="text-xs"
          data-testid={`mcp-groups-empty-${serverId}`}
        >
          No groups assigned
        </Text>
      ) : (
        <Flex vertical gap="small" className="w-full">
          <Text type="secondary" className="text-xs">
            User groups that have access to this MCP server
          </Text>
          <Space wrap size="small">
            {assignedGroups.map(group => (
              <Tag variant="outline"
                key={group.id}
                tone="info"
                className="text-[13px] px-2 py-1"
                data-testid={`mcp-group-tag-${group.id}`}
              >
                {group.name}
              </Tag>
            ))}
          </Space>
        </Flex>
      )}
    </div>
  )
}
