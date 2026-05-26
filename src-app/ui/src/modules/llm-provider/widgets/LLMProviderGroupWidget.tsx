import { useEffect } from 'react'
import { Button, Card, Flex, Space, Tag, Typography, Spin } from 'antd'
import { DatabaseOutlined, EditOutlined } from '@ant-design/icons'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Widget that displays LLM Providers assigned to a group.
 * Shows in GroupListItem below group info.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 *
 * IMPORTANT: Widget fetches data on mount AND listens to events for real-time updates.
 * This ensures data is loaded even after page reloads.
 */
export function LLMProviderGroupWidget({ group }: GroupWidgetProps) {
  // Get data from store
  const groupData = Stores.LlmProviderGroupWidget.groupProviders.get(group.id)
  const providers = groupData?.providers || []
  const loading = groupData?.loading || false
  const error = groupData?.error || null

  // CRITICAL: Load data on mount
  // The store has 30-second caching, so this won't cause excessive API calls
  useEffect(() => {
    Stores.LlmProviderGroupWidget.loadProvidersForGroup(group.id)
  }, [group.id])

  const handleEdit = () => {
    Stores.GroupLlmProvidersAssignment.openDrawer(group)
  }

  return (
    <Card data-widget="llm-providers" data-group-id={group.id}>
      <Flex vertical gap="small" style={{ width: '100%' }}>
        {/* Header */}
        <div className="flex items-center justify-between">
          <Space size="small">
            <DatabaseOutlined className="text-blue-500" aria-hidden="true" />
            <Text strong>LLM Providers</Text>
            {loading ? (
              <Spin size="small" />
            ) : (
              <Text type="secondary">({providers.length})</Text>
            )}
          </Space>
          <Button
            size="small"
            type="link"
            icon={<EditOutlined aria-hidden="true" />}
            onClick={handleEdit}
            aria-label={`Edit LLM Providers for ${group.name}`}
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
            Loading providers...
          </Text>
        ) : providers.length === 0 ? (
          <Text type="secondary" style={{ fontSize: '12px' }}>
            No providers assigned
          </Text>
        ) : (
          <Space wrap size="small" data-testid="provider-tags-container">
            {providers.map(provider => (
              <Tag
                key={provider.id}
                color={provider.enabled ? 'blue' : 'default'}
                style={{ fontSize: '11px' }}
              >
                {provider.name}
                {provider.built_in && ' (Built-in)'}
              </Tag>
            ))}
          </Space>
        )}
      </Flex>
    </Card>
  )
}
