import { Pencil } from 'lucide-react'
import { Button, Card, Flex, Spin, Tag, Text } from '@/components/ui'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'

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
      data-testid="llm-provider-groups-card"
      extra={
        <Button
          variant="outline"
          size="icon"
          icon={<Pencil aria-hidden="true" />}
          onClick={handleManageGroups}
          tooltip="Manage user groups"
          aria-label="Manage user groups"
          data-testid="llm-provider-groups-manage-btn"
        />
      }
    >
      {providerData?.loading ? (
        <Flex justify="center" align="center" className="p-5">
          <Spin label="Loading" />
        </Flex>
      ) : providerData?.error ? (
        <Text type="danger">{providerData.error}</Text>
      ) : providerData?.groups && providerData.groups.length > 0 ? (
        <Flex gap="middle" wrap>
          {providerData.groups.map(group => (
            <Tag variant="outline" key={group.id} tone="info" data-testid={`llm-provider-assigned-group-tag-${group.id}`}>
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
