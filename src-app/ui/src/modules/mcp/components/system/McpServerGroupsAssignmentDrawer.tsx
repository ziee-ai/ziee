import { useEffect, useState } from 'react'
import { App, Button, Card, Flex, Spin, Switch, Tag, Typography } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type McpServer } from '@/api-client/types'
import { emitMcpServerGroupsChanged } from '@/modules/mcp/events'

const { Text, Title } = Typography

/**
 * Drawer for assigning/removing user groups to/from a system MCP server.
 * Matches ProviderGroupAssignmentDrawer pattern exactly.
 */
export function McpServerGroupsAssignmentDrawer() {
  const { message } = App.useApp()
  const { isOpen, selectedServerId } = Stores.McpServerGroupsAssignment
  const { groups } = Stores.UserGroups

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const canManage = usePermission(Permissions.McpServersAdminEdit)

  // Load assigned groups when drawer opens
  useEffect(() => {
    if (isOpen && selectedServerId) {
      loadAssignedGroups()
    }
  }, [isOpen, selectedServerId])

  const loadAssignedGroups = async () => {
    if (!selectedServerId) return

    setLoading(true)
    try {
      const groupIds = await Stores.SystemMcpServer.getServerGroups(
        selectedServerId,
      )
      setAssignedIds(groupIds)
    } catch (error) {
      console.error('Failed to load assigned groups:', error)
      message.error('Failed to load assigned groups')
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!selectedServerId) return

    setSaving(true)
    try {
      // Use POST endpoint to replace all groups
      await Stores.SystemMcpServer.assignServerToGroups(
        selectedServerId,
        assignedIds,
      )

      message.success('Group assignments updated')

      // Emit event to invalidate cache
      await emitMcpServerGroupsChanged(selectedServerId, assignedIds)

      Stores.McpServerGroupsAssignment.closeDrawer()
    } catch (error) {
      console.error('Failed to update group assignments:', error)
      message.error('Failed to update group assignments')
    } finally {
      setSaving(false)
    }
  }

  const handleClose = () => {
    Stores.McpServerGroupsAssignment.closeDrawer()
  }

  const handleToggle = (groupId: string, checked: boolean) => {
    setAssignedIds(prev =>
      checked ? [...prev, groupId] : prev.filter(id => id !== groupId),
    )
  }

  const selectedServer = Stores.SystemMcpServer.systemServers.find(
    (s: McpServer) => s.id === selectedServerId,
  )

  return (
    <Drawer
      title={`Assign User Groups - ${selectedServer?.display_name || ''}`}
      open={isOpen}
      onClose={handleClose}
      size={600}
      footer={
        <div className="flex justify-end gap-2">
          <Button onClick={handleClose} disabled={saving}>
            {canManage ? 'Cancel' : 'Close'}
          </Button>
          {canManage && (
            <Button
              type="primary"
              onClick={handleSave}
              loading={saving}
              disabled={loading}
            >
              Save
            </Button>
          )}
        </div>
      }
    >
      {loading ? (
        <div className="flex justify-center p-8">
          <Spin />
        </div>
      ) : (
        <Flex vertical gap="large" style={{ width: '100%' }}>
          <div>
            <Title level={5} style={{ marginBottom: '8px' }}>
              Available Groups
            </Title>
            <Text type="secondary">
              Select which groups can access this server
            </Text>
          </div>

          {groups.length === 0 ? (
            <div className="p-4 text-center">
              <Text type="secondary">No groups available</Text>
            </div>
          ) : (
            <Flex vertical gap="middle" style={{ width: '100%' }}>
              {groups.map(group => {
                const isChecked = assignedIds.includes(group.id)
                return (
                  <Card key={group.id}>
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          checked={isChecked}
                          onChange={checked => handleToggle(group.id, checked)}
                          disabled={!canManage}
                          style={{ marginTop: '2px' }}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong style={{ fontSize: '14px' }}>
                            {group.name}
                          </Text>
                          {group.is_default && (
                            <Tag
                              color="blue"
                              style={{ fontSize: '11px', margin: 0 }}
                            >
                              Default
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
            </Flex>
          )}
        </Flex>
      )}
    </Drawer>
  )
}
