import { Database, Pencil } from 'lucide-react'
import { useEffect } from 'react'
import { Button, Card, Flex, Space, Tag, Text, Spin } from '@/components/ui'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'

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
    <Card data-widget="llm-providers" data-group-id={group.id} data-testid={`llm-provider-group-widget-card-${group.id}`}>
      <Flex vertical gap="small" className="w-full">
        {/* Header */}
        <div className="flex items-center justify-between">
          <Space size="small">
            <Database className="text-primary" aria-hidden="true" />
            <Text strong>LLM Providers</Text>
            {loading ? (
              <Spin size="sm" label="Loading" />
            ) : (
              <Text type="secondary">({providers.length})</Text>
            )}
          </Space>
          <Button
            size="default"
            variant="outline"
            icon={<Pencil aria-hidden="true" />}
            onClick={handleEdit}
            aria-label={`Edit LLM Providers for ${group.name}`}
            data-testid={`llm-provider-group-widget-edit-btn-${group.id}`}
          >
            Edit
          </Button>
        </div>

        {/* Content */}
        {error ? (
          <Text type="danger" className="text-xs">
            {error}
          </Text>
        ) : loading ? (
          <Text type="secondary" className="text-xs">
            Loading providers...
          </Text>
        ) : providers.length === 0 ? (
          <Text type="secondary" className="text-xs">
            No providers assigned
          </Text>
        ) : (
          <Space wrap size="small" data-testid="provider-tags-container">
            {providers.map(provider => (
              <Tag
                key={provider.id}
                tone={provider.enabled ? 'info' : undefined}
                className="text-xs"
                data-testid={`llm-provider-group-widget-tag-${provider.id}`}
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
