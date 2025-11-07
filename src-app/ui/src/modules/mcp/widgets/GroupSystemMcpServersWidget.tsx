import { useEffect } from 'react'
import { Button, Space, Tag, Typography, Spin } from 'antd'
import { ApiOutlined, EditOutlined } from '@ant-design/icons'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Widget that displays System MCP Servers assigned to a group.
 * Shows in GroupListItem below group info.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 */
export function GroupSystemMcpServersWidget({ group }: GroupWidgetProps) {
  // Get data from store
  const serverData = Stores.GroupSystemMcpServersWidget.groupServers.get(group.id)
  const servers = serverData?.servers || []
  const loading = serverData?.loading || false
  const error = serverData?.error || null

  const { lastUpdated } = Stores.GroupSystemMcpServersAssignment

  // Load servers on mount and when lastUpdated changes
  useEffect(() => {
    // Force reload when lastUpdated changes, otherwise use cached data
    Stores.GroupSystemMcpServersWidget.loadServersForGroup(group.id, !!lastUpdated)
  }, [group.id, lastUpdated])

  const handleEdit = () => {
    Stores.GroupSystemMcpServersAssignment.openDrawer(group)
  }

  return (
    <div className="p-3 bg-gray-50 dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-700">
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
    </div>
  )
}
