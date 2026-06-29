import { useEffect, useState } from 'react'
import { Button, Card, Flex, Spin, Switch, Tag, Text, Title, message } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'

/**
 * Drawer for assigning/removing LLM Providers to/from a group.
 * Self-contained - owned by LLM Provider module.
 */
export function GroupLlmProvidersAssignmentDrawer() {
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
      size={600}
      footer={
        <div className="flex justify-end gap-2">
          <Button onClick={handleClose} disabled={saving} data-testid="llm-group-providers-cancel-btn">
            Cancel
          </Button>
          <Button
            variant="default"
            onClick={handleSave}
            loading={saving}
            disabled={loading}
            data-testid="llm-group-providers-save-btn"
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
        <Flex direction="column" gap="large" className="w-full">
          <div>
            <Title level={5} className="mb-2">
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
            <Flex direction="column" gap="middle" className="w-full">
              {providers.map(provider => {
                const isChecked = assignedIds.includes(provider.id)
                return (
                  <Card
                    key={provider.id}
                    onClick={() => handleToggle(provider.id, !isChecked)}
                    className="cursor-pointer"
                    data-testid={`llm-group-provider-card-${provider.id}`}
                  >
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          checked={isChecked}
                          onChange={checked =>
                            handleToggle(provider.id, checked)
                          }
                          size="sm"
                          data-testid={`llm-group-provider-switch-${provider.id}`}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong className="text-sm">
                            {provider.name}
                          </Text>
                          {provider.built_in && (
                            <Tag
                              tone="info"
                              className="text-[11px] m-0"
                              data-testid={`llm-group-provider-builtin-tag-${provider.id}`}
                            >
                              Built-in
                            </Tag>
                          )}
                          {provider.enabled ? (
                            <Tag
                              tone="success"
                              className="text-[11px] m-0"
                              data-testid={`llm-group-provider-status-tag-${provider.id}`}
                            >
                              Enabled
                            </Tag>
                          ) : (
                            <Tag
                              tone="warning"
                              className="text-[11px] m-0"
                              data-testid={`llm-group-provider-status-tag-${provider.id}`}
                            >
                              Disabled
                            </Tag>
                          )}
                        </div>
                        <Text type="secondary" className="text-xs">
                          {provider.provider_type}
                        </Text>
                      </div>
                    </div>
                  </Card>
                )
              })}
            </Flex>
          )}
        </Flex>
      )}
    </Drawer>
  )
}
