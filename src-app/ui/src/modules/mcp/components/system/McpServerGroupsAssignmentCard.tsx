import { useEffect } from 'react'
import { Button, Card, Empty, Space, Tag, Typography } from 'antd'
import { EditOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

const { Text } = Typography

interface McpServerGroupsAssignmentCardProps {
  serverId: string
}

/**
 * Card for managing which user groups have access to a system MCP server.
 * Displays assigned groups and opens a drawer for management.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 *
 * IMPORTANT: Card fetches data on mount AND listens to events for real-time updates.
 * This ensures data is loaded even after page reloads.
 */
export function McpServerGroupsAssignmentCard({ serverId }: McpServerGroupsAssignmentCardProps) {
  // Get data from store
  const serverData = Stores.SystemMcpServerGroupCard.serverGroups.get(serverId)
  const assignedGroups = serverData?.groups || []
  const loading = serverData?.loading || false

  // CRITICAL: Load data on mount
  // The store has 30-second caching, so this won't cause excessive API calls
  useEffect(() => {
    Stores.SystemMcpServerGroupCard.loadGroupsForServer(serverId)
  }, [serverId])

  const handleManageGroups = () => {
    Stores.McpServerGroupsAssignment.openDrawer(serverId)
  }

  return (
    <Card
      title="User Groups"
      extra={
        <Button
          type="text"
          icon={<EditOutlined aria-hidden="true" />}
          onClick={handleManageGroups}
          aria-label="Manage user groups"
        />
      }
      loading={loading}
    >
      {assignedGroups.length === 0 ? (
        <Empty
          description="No groups assigned"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      ) : (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <Text type="secondary">
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
    </Card>
  )
}
