import { useEffect } from 'react'
import { Button, Empty, Space, Spin, Tag, Typography } from 'antd'
import { EditOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

const { Text } = Typography

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

  useEffect(() => {
    Stores.SystemMcpServerGroupCard.loadGroupsForServer(serverId)
  }, [serverId])

  const handleManageGroups = () => {
    Stores.McpServerGroupsAssignment.openDrawer(serverId)
  }

  return (
    <div
      className="border-t px-4 py-3"
      data-server-id={serverId}
      data-card-type="user-groups-assignment"
    >
      <div className="flex items-center justify-between mb-2">
        <Text className="font-medium text-sm">User Groups</Text>
        <Button
          type="text"
          size="small"
          icon={<EditOutlined aria-hidden="true" />}
          onClick={handleManageGroups}
          aria-label="Manage user groups"
        />
      </div>
      {loading ? (
        <Spin size="small" />
      ) : assignedGroups.length === 0 ? (
        <Empty
          description="No groups assigned"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          className="!my-2"
        />
      ) : (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <Text type="secondary" className="text-xs">
            User groups that have access to this MCP server
          </Text>
          <Space wrap size="small">
            {assignedGroups.map(group => (
              <Tag
                key={group.id}
                color="blue"
                style={{ fontSize: '13px', padding: '4px 8px' }}
              >
                {group.name}
              </Tag>
            ))}
          </Space>
        </Space>
      )}
    </div>
  )
}
