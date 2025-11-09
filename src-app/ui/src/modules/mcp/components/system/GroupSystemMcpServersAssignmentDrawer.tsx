import { useEffect, useState } from 'react'
import { App, Button, Card, Space, Spin, Switch, Tag, Typography } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import type { McpServer } from '@/api-client/types'

const { Text, Title } = Typography

/**
 * Drawer for assigning/removing system MCP servers to/from a group.
 * Self-contained - owned by MCP module.
 */
export function GroupSystemMcpServersAssignmentDrawer() {
  const { message } = App.useApp()
  const { isOpen, selectedGroup } = Stores.GroupSystemMcpServersAssignment
  const { systemServers } = Stores.SystemMcpServer

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)

  // Load assigned servers when drawer opens
  useEffect(() => {
    if (isOpen && selectedGroup) {
      loadAssignedServers()
    }
  }, [isOpen, selectedGroup])

  const loadAssignedServers = async () => {
    if (!selectedGroup) return

    setLoading(true)
    try {
      const assigned = await Stores.SystemMcpServer.getServersForGroup(
        selectedGroup.id,
      )
      setAssignedIds(assigned.map(s => s.id))
    } catch (error) {
      console.error('Failed to load assigned servers:', error)
      message.error('Failed to load assigned servers')
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!selectedGroup) return

    setSaving(true)
    try {
      await Stores.SystemMcpServer.updateGroupServers(
        selectedGroup.id,
        assignedIds,
      )
      message.success('Server assignments updated')
      Stores.GroupSystemMcpServersAssignment.closeDrawer()
    } catch (error) {
      console.error('Failed to update server assignments:', error)
      message.error('Failed to update server assignments')
    } finally {
      setSaving(false)
    }
  }

  const handleClose = () => {
    Stores.GroupSystemMcpServersAssignment.closeDrawer()
  }

  const handleToggle = (serverId: string, checked: boolean) => {
    setAssignedIds(prev =>
      checked ? [...prev, serverId] : prev.filter(id => id !== serverId),
    )
  }

  return (
    <Drawer
      title={`Assign System MCP Servers - ${selectedGroup?.name || ''}`}
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
              Available Servers
            </Title>
            <Text type="secondary">
              Select which servers this group can access
            </Text>
          </div>

          {systemServers.length === 0 ? (
            <div className="p-4 text-center">
              <Text type="secondary">No system servers available</Text>
            </div>
          ) : (
            <Space direction="vertical" size="middle" style={{ width: '100%' }}>
              {systemServers.map((server: McpServer) => {
                const isChecked = assignedIds.includes(server.id)
                return (
                  <Card
                    key={server.id}
                    style={{ cursor: 'pointer' }}
                    onClick={() => handleToggle(server.id, !isChecked)}
                  >
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          checked={isChecked}
                          onChange={checked => handleToggle(server.id, checked)}
                          style={{ marginTop: '2px' }}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong style={{ fontSize: '14px' }}>
                            {server.display_name}
                          </Text>
                          <Tag color="purple" style={{ fontSize: '11px', margin: 0 }}>
                            {server.transport_type}
                          </Tag>
                          {server.enabled ? (
                            <Tag color="green" style={{ fontSize: '11px', margin: 0 }}>
                              Enabled
                            </Tag>
                          ) : (
                            <Tag color="orange" style={{ fontSize: '11px', margin: 0 }}>
                              Disabled
                            </Tag>
                          )}
                        </div>
                        {server.description && (
                          <Text type="secondary" style={{ fontSize: '12px' }}>
                            {server.description}
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
