import { useEffect, useState } from 'react'
import { App, Button, Space, Spin, Switch, Tag, Typography } from 'antd'
import { Drawer } from '@/components/common/Drawer'
import { Stores } from '@/core/stores'

const { Text, Title } = Typography

/**
 * Drawer for assigning/removing LLM Providers to/from a group.
 * Self-contained - owned by LLM Provider module.
 */
export function GroupLlmProvidersAssignmentDrawer() {
  const { message } = App.useApp()
  const { isOpen, selectedGroup } = Stores.GroupLlmProvidersAssignment
  const { providers } = Stores.LlmProvider

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)

  // Load assigned providers when drawer opens
  useEffect(() => {
    if (isOpen && selectedGroup) {
      loadAssignedProviders()
    }
  }, [isOpen, selectedGroup])

  const loadAssignedProviders = async () => {
    if (!selectedGroup) return

    setLoading(true)
    try {
      const assigned = await Stores.LlmProvider.getProvidersForGroup(
        selectedGroup.id,
      )
      setAssignedIds(assigned.map(p => p.id))
    } catch (error) {
      console.error('Failed to load assigned providers:', error)
      message.error('Failed to load assigned providers')
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!selectedGroup) return

    setSaving(true)
    try {
      await Stores.LlmProvider.updateGroupProviders(
        selectedGroup.id,
        assignedIds,
      )
      message.success('Provider assignments updated')
      Stores.GroupLlmProvidersAssignment.closeDrawer()
    } catch (error) {
      console.error('Failed to update provider assignments:', error)
      message.error('Failed to update provider assignments')
    } finally {
      setSaving(false)
    }
  }

  const handleClose = () => {
    Stores.GroupLlmProvidersAssignment.closeDrawer()
  }

  const handleToggle = (providerId: string, checked: boolean) => {
    setAssignedIds(prev =>
      checked ? [...prev, providerId] : prev.filter(id => id !== providerId),
    )
  }

  return (
    <Drawer
      title={`Assign LLM Providers - ${selectedGroup?.name || ''}`}
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
              Available Providers
            </Title>
            <Text type="secondary">
              Select which providers this group can access
            </Text>
          </div>

          {providers.length === 0 ? (
            <div className="p-4 text-center">
              <Text type="secondary">No providers available</Text>
            </div>
          ) : (
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              {providers.map(provider => {
                const isChecked = assignedIds.includes(provider.id)
                return (
                  <div
                    key={provider.id}
                    className="p-3 rounded border border-gray-200 dark:border-gray-700 hover:border-blue-400 dark:hover:border-blue-600 transition-colors cursor-pointer"
                    onClick={() => handleToggle(provider.id, !isChecked)}
                  >
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          checked={isChecked}
                          onChange={checked => handleToggle(provider.id, checked)}
                          style={{ marginTop: '2px' }}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong style={{ fontSize: '14px' }}>
                            {provider.name}
                          </Text>
                          {provider.built_in && (
                            <Tag color="blue" style={{ fontSize: '11px', margin: 0 }}>
                              Built-in
                            </Tag>
                          )}
                          {provider.enabled ? (
                            <Tag color="green" style={{ fontSize: '11px', margin: 0 }}>
                              Enabled
                            </Tag>
                          ) : (
                            <Tag color="orange" style={{ fontSize: '11px', margin: 0 }}>
                              Disabled
                            </Tag>
                          )}
                        </div>
                        <Text type="secondary" style={{ fontSize: '12px' }}>
                          {provider.provider_type}
                        </Text>
                      </div>
                    </div>
                  </div>
                )
              })}
            </Space>
          )}
        </Space>
      )}
    </Drawer>
  )
}
