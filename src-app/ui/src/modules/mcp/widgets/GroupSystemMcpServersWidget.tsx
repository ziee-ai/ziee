import { useEffect } from 'react'
import { Button, Card, Space, Tag, Typography, Spin } from 'antd'
import { ApiOutlined, EditOutlined } from '@ant-design/icons'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Widget that displays System MCP Servers assigned to a group.
 * Shows in GroupListItem below group info.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 *
 * IMPORTANT: Widget fetches data on mount AND listens to events for real-time updates.
 * This ensures data is loaded even after page reloads.
 */
export function GroupSystemMcpServersWidget({ group }: GroupWidgetProps) {
  // Get data from store
  const serverData = Stores.GroupSystemMcpServersWidget.groupServers.get(
    group.id,
  )
  const servers = serverData?.servers || []
  const loading = serverData?.loading || false
  const error = serverData?.error || null

  // CRITICAL: Load data on mount
  // The store has 30-second caching, so this won't cause excessive API calls
  useEffect(() => {
    Stores.GroupSystemMcpServersWidget.loadServersForGroup(group.id)
  }, [group.id])

  const handleEdit = () => {
    Stores.GroupSystemMcpServersAssignment.openDrawer(group)
  }

  return (
    <Card data-widget="system-mcp-servers" data-group-id={group.id}>
      <Space direction="vertical" size="small" style={{ width: '100%' }}>
        {/* Header */}
        <div className="flex items-center justify-between">
          <Space size="small">
            <ApiOutlined className="text-blue-500" aria-hidden="true" />
            <Text strong>System MCP Servers</Text>
            {loading ? (
              <Spin size="small" />
            ) : (
              <Text type="secondary">({servers.length})</Text>
            )}
          </Space>
          <Button
            size="small"
            type="link"
            icon={<EditOutlined aria-hidden="true" />}
            onClick={handleEdit}
            aria-label={`Edit System MCP Servers for ${group.name}`}
          >
            Edit
          </Button>
        </div>

        {/* Content */}
        {error ? (
          <Text type="danger" style={{ fontSize: '12px' }}>
            {error}
          </Text>
        ) : loading ? (
          <Text type="secondary" style={{ fontSize: '12px' }}>
            Loading servers...
          </Text>
        ) : servers.length === 0 ? (
          <Text type="secondary" style={{ fontSize: '12px' }}>
            No servers assigned
          </Text>
        ) : (
          <Space wrap size="small">
            {servers.map(server => (
              <Tag
                key={server.id}
                color={server.enabled ? 'blue' : 'default'}
                style={{ fontSize: '11px' }}
              >
                {server.display_name}
              </Tag>
            ))}
          </Space>
        )}
      </Space>
    </Card>
  )
}
