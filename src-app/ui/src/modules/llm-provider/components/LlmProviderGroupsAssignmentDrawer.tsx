import { useEffect, useState } from 'react'
import { App, Button, Card, Space, Spin, Switch, Tag, Typography } from 'antd'
import { Drawer } from '@/components/common/Drawer'
import { Stores } from '@/core/stores'
import { emitLlmProviderGroupsChanged } from '../events'

const { Text, Title } = Typography

/**
 * Drawer for assigning/removing user groups to/from an LLM provider.
 * Self-contained - owned by LLM Provider module.
 */
export function LlmProviderGroupsAssignmentDrawer() {
  const { message } = App.useApp()
  const { isOpen, selectedProviderId } = Stores.LlmProviderGroupsAssignment
  const { groups } = Stores.UserGroups

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)

  // Get the current provider name for display
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === selectedProviderId
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
      const assigned = await Stores.LlmProvider.getGroupsForProvider(
        selectedProviderId,
      )
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
      const currentGroups = await Stores.LlmProvider.getGroupsForProvider(selectedProviderId)
      const currentIds = new Set(currentGroups.map(g => g.id))
      const newIds = new Set(assignedIds)

      // Determine what to add and remove
      const toAdd = assignedIds.filter(id => !currentIds.has(id))
      const toRemove = Array.from(currentIds).filter(id => !newIds.has(id))

      // Add new groups
      for (const groupId of toAdd) {
        await Stores.LlmProvider.assignGroupToProvider(selectedProviderId, groupId)
      }

      // Remove unassigned groups
      for (const groupId of toRemove) {
        await Stores.LlmProvider.removeGroupFromProvider(selectedProviderId, groupId)
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
      width={600}
      footer={
        <div className="flex justify-end gap-2">
          <Button onClick={handleClose} disabled={saving}>
            Cancel
          </Button>
          <Button
            type="primary"
            onClick={handleSave}
            loading={saving}
            disabled={loading}
          >
            Save
          </Button>
        </div>
      }
    >
      {loading ? (
        <div className="flex justify-center p-8">
          <Spin />
        </div>
      ) : (
        <Space direction="vertical" size="large" style={{ width: '100%' }}>
          <div>
            <Title level={5} style={{ marginBottom: '8px' }}>
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
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              {groups.map(group => {
                const isChecked = assignedIds.includes(group.id)
                return (
                  <Card
                    key={group.id}
                    hoverable
                    onClick={() => handleToggle(group.id, !isChecked)}
                  >
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          checked={isChecked}
                          onChange={checked => handleToggle(group.id, checked)}
                          style={{ marginTop: '2px' }}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong style={{ fontSize: '14px' }}>
                            {group.name}
                          </Text>
                          {group.is_system && (
                            <Tag color="orange" style={{ fontSize: '11px', margin: 0 }}>
                              System
                            </Tag>
                          )}
                          {group.is_active ? (
                            <Tag color="green" style={{ fontSize: '11px', margin: 0 }}>
                              Active
                            </Tag>
                          ) : (
                            <Tag color="default" style={{ fontSize: '11px', margin: 0 }}>
                              Inactive
                            </Tag>
                          )}
                        </div>
                        {group.description && (
                          <Text type="secondary" style={{ fontSize: '12px' }}>
                            {group.description}
                          </Text>
                        )}
                      </div>
                    </div>
                  </Card>
                )
              })}
            </Space>
          )}
        </Space>
      )}
    </Drawer>
  )
}
