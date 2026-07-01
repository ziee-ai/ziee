import { useEffect, useState } from 'react'
import { Button, Card, Spin, Switch, Tag, message } from '@/components/ui'
import { Text, Title } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { emitLlmProviderGroupsChanged } from '@/modules/llm-provider/events'

/**
 * Drawer for assigning/removing user groups to/from an LLM provider.
 * Self-contained - owned by LLM Provider module.
 */
export function LlmProviderGroupsAssignmentDrawer() {
  const { isOpen, selectedProviderId } = Stores.LlmProviderGroupsAssignment
  const { groups } = Stores.UserGroups

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)

  // Get the current provider name for display
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === selectedProviderId,
  )

  // Load assigned groups when drawer opens
  useEffect(() => {
    if (isOpen && selectedProviderId) {
      loadAssignedGroups()
    }
  }, [isOpen, selectedProviderId])

  const loadAssignedGroups = async () => {
    if (!selectedProviderId) return

    setLoading(true)
    try {
      const assigned =
        await Stores.LlmProvider.getGroupsForProvider(selectedProviderId)
      setAssignedIds(assigned.map(g => g.id))
    } catch (error) {
      console.error('Failed to load assigned groups:', error)
      message.error('Failed to load assigned groups')
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!selectedProviderId) return

    setSaving(true)
    try {
      // Get current assignments
      const currentGroups =
        await Stores.LlmProvider.getGroupsForProvider(selectedProviderId)
      const currentIds = new Set(currentGroups.map(g => g.id))
      const newIds = new Set(assignedIds)

      // Determine what to add and remove
      const toAdd = assignedIds.filter(id => !currentIds.has(id))
      const toRemove = Array.from(currentIds).filter(id => !newIds.has(id))

      // Add new groups
      for (const groupId of toAdd) {
        await Stores.LlmProvider.assignGroupToProvider(
          selectedProviderId,
          groupId,
        )
      }

      // Remove unassigned groups
      for (const groupId of toRemove) {
        await Stores.LlmProvider.removeGroupFromProvider(
          selectedProviderId,
          groupId,
        )
      }

      message.success('Group assignments updated')

      // Emit event to invalidate provider group cache
      await emitLlmProviderGroupsChanged(selectedProviderId, assignedIds)

      Stores.LlmProviderGroupsAssignment.closeDrawer()
    } catch (error) {
      console.error('Failed to update group assignments:', error)
      message.error('Failed to update group assignments')
    } finally {
      setSaving(false)
    }
  }

  const handleClose = () => {
    Stores.LlmProviderGroupsAssignment.closeDrawer()
  }

  const handleToggle = (groupId: string, checked: boolean) => {
    setAssignedIds(prev =>
      checked ? [...prev, groupId] : prev.filter(id => id !== groupId),
    )
  }

  return (
    <Drawer
      title={`Assign User Groups - ${currentProvider?.name || ''}`}
      open={isOpen}
      onClose={handleClose}
      className="!max-w-[600px]"
      footer={
        <div className="flex justify-end gap-2">
          <Button onClick={handleClose} disabled={saving} data-testid="llm-provider-groups-cancel-btn">
            Cancel
          </Button>
          <Button
            variant="default"
            onClick={handleSave}
            loading={saving}
            disabled={loading}
            data-testid="llm-provider-groups-save-btn"
          >
            Save
          </Button>
        </div>
      }
    >
      {loading ? (
        <div className="flex justify-center p-8">
          <Spin label="Loading" />
        </div>
      ) : (
        <div className="flex flex-col gap-5 w-full">
          <div className="mb-2">
            <Title level={5} className="mb-2">
              Available Groups
            </Title>
            <Text type="secondary">
              Select which groups can access this provider
            </Text>
          </div>

          {groups.length === 0 ? (
            <div className="p-4 text-center">
              <Text type="secondary">No groups available</Text>
            </div>
          ) : (
            <div className="flex flex-col gap-3 w-full">
              {groups.map(group => {
                const isChecked = assignedIds.includes(group.id)
                return (
                  <Card key={group.id} className="w-full" data-testid={`llm-provider-group-card-${group.id}`}>
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          tooltip="Assign this group"
                          checked={isChecked}
                          onChange={checked => handleToggle(group.id, checked)}
                          className="mt-0.5"
                          data-testid={`llm-provider-group-switch-${group.id}`}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong className="text-sm">
                            {group.name}
                          </Text>
                          {group.is_system && (
                            <Tag
                              tone="warning"
                              className="text-xs m-0"
                              data-testid={`llm-provider-group-system-tag-${group.id}`}
                            >
                              System
                            </Tag>
                          )}
                          {group.is_active ? (
                            <Tag
                              tone="success"
                              className="text-xs m-0"
                              data-testid={`llm-provider-group-status-tag-${group.id}`}
                            >
                              Active
                            </Tag>
                          ) : (
                            <Tag
                              className="text-xs m-0"
                              data-testid={`llm-provider-group-status-tag-${group.id}`}
                            >
                              Inactive
                            </Tag>
                          )}
                        </div>
                        {group.description && (
                          <Text type="secondary" className="text-xs">
                            {group.description}
                          </Text>
                        )}
                      </div>
                    </div>
                  </Card>
                )
              })}
            </div>
          )}
        </div>
      )}
    </Drawer>
  )
}
