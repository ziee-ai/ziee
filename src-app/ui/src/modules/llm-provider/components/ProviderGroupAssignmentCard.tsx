import { Button, Card, Flex, Spin, Tag, Typography } from 'antd'
import { EditOutlined } from '@ant-design/icons'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Card for managing which user groups have access to an LLM provider.
 * Shows assigned groups and allows opening a drawer to modify assignments.
 */
export function ProviderGroupAssignmentCard() {
  const { providerId } = useParams<{ providerId?: string }>()

  // ✅ CORRECT: Destructure all needed values at top level
  const { providerGroups } =
    Stores.ProviderGroupAssignmentCard
  const { openDrawer } = Stores.LlmProviderGroupsAssignment

  const providerData = providerId ? providerGroups.get(providerId) : undefined

  // Load groups for this provider on mount
  useEffect(() => {
    if (providerId) {
      Stores.ProviderGroupAssignmentCard.loadGroupsForProvider(providerId)
    }
  }, [providerId])

  if (!providerId) {
    return null
  }

  const handleManageGroups = () => {
    openDrawer(providerId)
  }

  return (
    <Card
      title="User Groups"
      extra={
        <Button
          type="text"
          icon={<EditOutlined />}
          onClick={handleManageGroups}
          aria-label="Manage user groups"
        />
      }
    >
      {providerData?.loading ? (
        <Flex justify="center" align="center" style={{ padding: '20px' }}>
          <Spin />
        </Flex>
      ) : providerData?.error ? (
        <Text type="danger">{providerData.error}</Text>
      ) : providerData?.groups && providerData.groups.length > 0 ? (
        <Flex gap={8} wrap="wrap">
          {providerData.groups.map(group => (
            <Tag key={group.id} color="blue">
              {group.name}
            </Tag>
          ))}
        </Flex>
      ) : (
        <Text type="secondary">No groups assigned</Text>
      )}
    </Card>
  )
}
