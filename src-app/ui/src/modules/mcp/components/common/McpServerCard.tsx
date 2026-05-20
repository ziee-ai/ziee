import { useState } from 'react'
import { App, Button, Card, Tag, Typography, Tooltip, Switch, Flex } from 'antd'
import { EditOutlined, ToolOutlined, DeleteOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { McpServer } from '@/api-client/types'

const { Text } = Typography

interface McpServerCardProps {
  server: McpServer
  isEditable?: boolean
  bordered?: boolean
}

export function McpServerCard({
  server,
  isEditable = true,
  bordered = true,
}: McpServerCardProps) {
  const { message, modal } = App.useApp()
  const [enableLoading, setEnableLoading] = useState(false)

  const handleEdit = () => {
    if (server.is_system) {
      Stores.McpServerDrawer.openMcpServerDrawer(server, 'edit-system')
    } else {
      Stores.McpServerDrawer.openMcpServerDrawer(server, 'edit')
    }
  }

  const handleDelete = () => {
    if (server.enabled) {
      message.warning('Please disable the server before deleting it')
      return
    }

    modal.confirm({
      title: 'Delete Server',
      content: `Are you sure you want to delete "${server.display_name}"? This action cannot be undone.`,
      okText: 'Delete',
      okType: 'danger',
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          if (server.is_system) {
            await Stores.SystemMcpServer.deleteSystemServer(server.id)
          } else {
            await Stores.McpServer.deleteMcpServer(server.id)
          }
          message.success('Server deleted successfully')
        } catch (error) {
          message.error('Failed to delete server')
        }
      },
    })
  }

  const handleToggleEnable = async (enabled: boolean) => {
    setEnableLoading(true)
    try {
      if (server.is_system) {
        await Stores.SystemMcpServer.updateSystemServer(server.id, {
          enabled,
        })
      } else {
        await Stores.McpServer.updateMcpServer(server.id, {
          enabled,
        })
      }
      message.success(`Server ${enabled ? 'enabled' : 'disabled'} successfully`)
    } catch (error) {
      console.error('Failed to toggle server enable state:', error)
      message.error(`Failed to ${enabled ? 'enable' : 'disable'} server`)
    } finally {
      setEnableLoading(false)
    }
  }

  const headerBg =
    server.transport_type === 'stdio'
      ? 'bg-blue-50'
      : server.transport_type === 'http'
        ? 'bg-green-50'
        : 'bg-purple-50'

  return (
    <Card
      classNames={{
        body: '!p-3',
      }}
      bordered={bordered}
      data-testid={`mcp-server-card-${server.id}`}
    >
      <div className="flex items-start gap-3 flex-wrap">
        {/* Server Info */}
        <div className="flex-1">
          <div className={`-mx-3 -mt-3 mb-3 px-3 py-2 flex items-center gap-2 flex-wrap ${headerBg}`}>
            <div className="flex-1 min-w-48">
              <Flex className="gap-2 items-center">
                <ToolOutlined aria-hidden="true" className="text-base" />
                <Text className="font-semibold text-base">{server.display_name}</Text>
                {!isEditable && server.is_system && (
                  <Tag color="blue">System</Tag>
                )}
                <Tag
                  color={
                    server.transport_type === 'stdio'
                      ? 'blue'
                      : server.transport_type === 'http'
                        ? 'green'
                        : 'purple'
                  }
                >
                  {server.transport_type.toUpperCase()}
                </Tag>
                {server.supports_sampling && (
                  <Tooltip title={`Sampling enabled · ${server.usage_mode === 'always' ? 'Always mode' : 'Auto mode'}`}>
                    <Tag color="cyan" data-testid="mcp-sampling-badge">Sampling</Tag>
                  </Tooltip>
                )}
                {server.usage_mode === 'always' && (
                  <Tag color="orange" data-testid="mcp-always-badge">Always</Tag>
                )}
              </Flex>
            </div>
            <div className="flex gap-2 items-center justify-end">
              {isEditable && (
                <>
                  <Tooltip
                    title={server.enabled ? 'Disable Server' : 'Enable Server'}
                  >
                    <Switch
                      checked={server.enabled}
                      onChange={handleToggleEnable}
                      loading={enableLoading}
                      aria-label={`${server.enabled ? 'Disable' : 'Enable'} ${server.display_name}`}
                    />
                  </Tooltip>
                  <Button
                    icon={<EditOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleEdit()
                    }}
                    data-testid="mcp-server-edit-btn"
                  >
                    Edit
                  </Button>
                  {!server.is_built_in && (
                    <Button
                      icon={<DeleteOutlined />}
                      danger
                      onClick={e => {
                        e.stopPropagation()
                        handleDelete()
                      }}
                      aria-label={`Delete ${server.display_name}`}
                      data-testid="mcp-server-delete-btn"
                    >
                      Delete
                    </Button>
                  )}
                </>
              )}
            </div>
          </div>

          <div>
            <Text type="secondary" className="text-sm mb-2 block">
              {server.description || 'No description'}
            </Text>

            {/* Transport Information */}
            <div className="mb-2">
              {server.url && (
                <>
                  <Text type="secondary" className="text-xs mr-2">
                    URL:
                  </Text>
                  <Text type="secondary" className="text-xs truncate">
                    {server.url}
                  </Text>
                </>
              )}
              {server.command && (
                <Card size="small" className={'!mt-2'}>
                  <pre className="text-xs overflow-auto m-0">
                    {server.command}
                    {server.args &&
                      Array.isArray(server.args) &&
                      server.args.length > 0 && (
                        <span> {server.args.join(' ')}</span>
                      )}
                  </pre>
                </Card>
              )}
            </div>
          </div>
        </div>
      </div>
    </Card>
  )
}
