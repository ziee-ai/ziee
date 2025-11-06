import { useEffect } from 'react'
import { App, Button, Card, Empty, Space, Tag, Typography } from 'antd'
import { EditOutlined } from '@ant-design/icons'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Card for managing which user groups have access to an LLM provider.
 * Displays assigned groups and opens a drawer for management.
 * Uses a dedicated store to prevent duplicate API calls and cache data.
 */
export function ProviderGroupAssignmentCard() {
  const { message } = App.useApp()
  const { providerId } = useParams<{ providerId?: string }>()

  // Get data from store
  const providerData = providerId
    ? Stores.ProviderGroupCard.providerGroups.get(providerId)
    : undefined
  const assignedGroups = providerData?.groups || []
  const loading = providerData?.loading || false

  // Get lastUpdated from drawer store to watch for changes
  const { lastUpdated } = Stores.ProviderGroupAssignment

  // Load assigned groups when provider changes or drawer updates
  useEffect(() => {
    if (providerId) {
      // Force reload when lastUpdated changes, otherwise use cached data
      Stores.ProviderGroupCard.loadGroupsForProvider(providerId, !!lastUpdated).catch(err => {
        console.error('Failed to load assigned groups:', err)
        message.error('Failed to load assigned groups')
      })
    }
  }, [providerId, lastUpdated, message])

  const handleManageGroups = () => {
    if (!providerId) return
    Stores.ProviderGroupAssignment.openDrawer(providerId)
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
            User groups that have access to this LLM provider
          </Text>
          <Space wrap size="small">
            {assignedGroups.map((group: { id: string; name: string }) => (
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
