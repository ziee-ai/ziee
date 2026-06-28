import { useEffect } from 'react'
import { Pencil } from 'lucide-react'
import { Button, Accordion, Empty, Flex, Space, Spin, Tag, Text } from '@/components/ui'
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
      <Accordion
        ghost
        collapsible
        items={[
          {
            key: 'groups',
            label: <Text className="font-medium text-sm">User Groups</Text>,
            children: (
              <>
                {canManage && (
                  <div className="mb-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={<Pencil aria-hidden="true" />}
                      onClick={e => {
                        e.stopPropagation()
                        handleManageGroups()
                      }}
                      aria-label="Manage user groups"
                    >
                      Assign
                    </Button>
                  </div>
                )}
                {loading ? (
                  <Spin size="sm" label="Loading" />
                ) : assignedGroups.length === 0 ? (
                  <Empty
                    description="No groups assigned"
                    className="!my-2"
                  />
                ) : (
                  <Flex vertical gap="small" className="w-full">
                    <Text type="secondary" className="text-xs">
                      User groups that have access to this MCP server
                    </Text>
                    <Space wrap size="small">
                      {assignedGroups.map(group => (
                        <Tag
                          key={group.id}
                          tone="info"
                          className="text-[13px] px-2 py-1"
                        >
                          {group.name}
                        </Tag>
                      ))}
                    </Space>
                  </Flex>
                )}
              </>
            ),
          },
        ]}
      />
    </div>
  )
}
