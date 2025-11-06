import { useEffect } from 'react'
import { Button, Space, Tag, Typography, Spin } from 'antd'
import { DatabaseOutlined, EditOutlined } from '@ant-design/icons'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Widget that displays LLM Providers assigned to a group.
 * Shows in GroupListItem below group info.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 */
export function LLMProviderGroupWidget({ group }: GroupWidgetProps) {
  // Get data from store
  const groupData = Stores.LlmProviderGroupWidget.groupProviders.get(group.id)
  const providers = groupData?.providers || []
  const loading = groupData?.loading || false
  const error = groupData?.error || null

  const { lastUpdated } = Stores.LlmProviderGroupAssignment

  // Load providers on mount and when lastUpdated changes
  useEffect(() => {
    // Force reload when lastUpdated changes, otherwise use cached data
    Stores.LlmProviderGroupWidget.loadProvidersForGroup(group.id, !!lastUpdated)
  }, [group.id, lastUpdated])

  const handleEdit = () => {
    Stores.LlmProviderGroupAssignment.openDrawer(group)
  }

  return (
    <div className="p-3 bg-gray-50 dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-700">
      <Space direction="vertical" size="small" style={{ width: '100%' }}>
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
          <Space wrap size="small">
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
      </Space>
    </div>
  )
}
