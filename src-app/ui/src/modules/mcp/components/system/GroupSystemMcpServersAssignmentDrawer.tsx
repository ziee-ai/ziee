import { useEffect, useState } from 'react'
import { Button, Card, Flex, Spinner, Switch, Tag, Text, Title, message } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type McpServer } from '@/api-client/types'

/**
 * Drawer for assigning/removing system MCP servers to/from a group.
 * Self-contained - owned by MCP module.
 */
export function GroupSystemMcpServersAssignmentDrawer() {
  const { isOpen, selectedGroup } = Stores.GroupSystemMcpServersAssignment
  const { systemServers } = Stores.SystemMcpServer

  const [assignedIds, setAssignedIds] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const canManage = usePermission(Permissions.McpServersAdminEdit)

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
      size={600}
      footer={
        <div className="flex justify-end gap-2">
          <Button onClick={handleClose} disabled={saving} data-testid="mcp-group-assign-cancel-btn">
            {canManage ? 'Cancel' : 'Close'}
          </Button>
          {canManage && (
            <Button
              variant="default"
              onClick={handleSave}
              loading={saving}
              disabled={loading}
              data-testid="mcp-group-assign-save-btn"
            >
              Save
            </Button>
          )}
        </div>
      }
    >
      {loading ? (
        <div className="flex justify-center p-8">
          <Spinner label="Loading servers" />
        </div>
      ) : (
        <Flex direction="column" className="w-full gap-4">
          <div>
            <Title level={5} className="mb-2">
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
            <Flex direction="column" className="w-full gap-4">
              {systemServers.map((server: McpServer) => {
                const isChecked = assignedIds.includes(server.id)
                return (
                  <Card
                    key={server.id}
                    role="listitem"
                    data-cursor={canManage ? 'pointer' : 'default'}
                    data-testid={`mcp-group-assign-card-${server.id}`}
                    onClick={() =>
                      canManage && handleToggle(server.id, !isChecked)
                    }
                  >
                    <div className="flex items-start gap-3">
                      <div onClick={e => e.stopPropagation()}>
                        <Switch
                          tooltip="Assign this server"
                          checked={isChecked}
                          onChange={checked => handleToggle(server.id, checked)}
                          disabled={!canManage}
                          className="mt-0.5"
                          data-testid={`mcp-group-assign-switch-${server.id}`}
                        />
                      </div>
                      <div className="flex flex-col gap-1 flex-1">
                        <div className="flex items-center gap-2">
                          <Text strong className="text-sm">
                            {server.display_name}
                          </Text>
                          <Tag
                            tone="info"
                            variant="solid"
                            className="text-xs m-0"
                            data-testid={`mcp-group-assign-transport-tag-${server.id}`}
                          >
                            {server.transport_type}
                          </Tag>
                          {server.enabled ? (
                            <Tag
                              tone="success"
                              variant="solid"
                              className="text-xs m-0"
                              data-testid={`mcp-group-assign-status-tag-${server.id}`}
                            >
                              Enabled
                            </Tag>
                          ) : (
                            <Tag
                              tone="warning"
                              variant="solid"
                              className="text-xs m-0"
                              data-testid={`mcp-group-assign-status-tag-${server.id}`}
                            >
                              Disabled
                            </Tag>
                          )}
                        </div>
                        {server.description && (
                          <Text type="secondary" className="text-xs">
                            {server.description}
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
