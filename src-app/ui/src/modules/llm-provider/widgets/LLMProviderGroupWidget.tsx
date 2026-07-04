import { Database, Pencil } from 'lucide-react'
import { useEffect } from 'react'
import { Button, Card, Flex, Space, Tag, Text, Spin } from '@/components/ui'
import type { GroupWidgetProps } from '@/modules/user/types/GroupWidget'
import { Stores } from '@/core/stores'
import { LlmProviderGroupWidgetStore } from './LLMProviderGroupWidget.store'

/**
 * Widget that displays LLM Providers assigned to a group.
 *
 * Backed by a PRIVATE per-instance store (`defineLocalStore`) — one per group
 * row, fetched on mount, listeners scoped to this group and auto-cleaned on
 * unmount. Reactive reads use the same `const { … } = s` syntax as `Stores.X`.
 */
export function LLMProviderGroupWidget({ group }: GroupWidgetProps) {
  const s = LlmProviderGroupWidgetStore.use({ groupId: group.id })
  const { providers, loading, error } = s

  // Defensive re-point if this widget instance is reused for a different group
  // (no-op on mount since the initial groupId already matches).
  useEffect(() => {
    s.setGroup(group.id)
  }, [group.id, s])

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
            variant="ghost"
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
              <Tag variant="outline"
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
