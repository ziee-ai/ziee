import { useEffect } from 'react'
import { Button, Card, Flex, Space, Tag, Text, Spin } from '@/components/ui'
import { Plug, Pencil } from 'lucide-react'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

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
  const canManage = usePermission(Permissions.McpServersAdminEdit)

  // CRITICAL: Load data on mount
  // The store has 30-second caching, so this won't cause excessive API calls
  useEffect(() => {
    Stores.GroupSystemMcpServersWidget.loadServersForGroup(group.id)
  }, [group.id])

  const handleEdit = () => {
    Stores.GroupSystemMcpServersAssignment.openDrawer(group)
  }

  return (
    <Card data-widget="system-mcp-servers" data-group-id={group.id} data-testid={`mcp-group-widget-card-${group.id}`}>
      <Flex vertical gap="small" className="w-full">
        {/* Header */}
        <div className="flex items-center justify-between">
          <Space size="small">
            <Plug className="text-primary" aria-hidden="true" />
            <Text strong>System MCP Servers</Text>
            {loading ? (
              <Spin size="sm" label="Loading" />
            ) : (
              <Text type="secondary">({servers.length})</Text>
            )}
          </Space>
          {canManage && (
            <Button
              size="default"
              variant="ghost"
              icon={<Pencil aria-hidden="true" />}
              onClick={handleEdit}
              aria-label={`Edit System MCP Servers for ${group.name}`}
              data-testid={`mcp-group-widget-edit-btn-${group.id}`}
            >
              Edit
            </Button>
          )}
        </div>

        {/* Content */}
        {error ? (
          <Text type="danger" className="text-xs">
            {error}
          </Text>
        ) : loading ? (
          <Text type="secondary" className="text-xs">
            Loading servers...
          </Text>
        ) : servers.length === 0 ? (
          <Text type="secondary" className="text-xs">
            No servers assigned
          </Text>
        ) : (
          <Space wrap size="small">
            {servers.map(server => (
              <Tag
                key={server.id}
                tone={server.enabled ? 'info' : undefined}
                variant="outline"
                data-testid={`mcp-group-widget-server-tag-${server.id}`}
              >
                {server.display_name}
              </Tag>
            ))}
          </Space>
        )}
      </Flex>
    </Card>
  )
}
