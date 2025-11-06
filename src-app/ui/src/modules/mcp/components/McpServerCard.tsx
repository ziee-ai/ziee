import { useState } from 'react'
import { App, Button, Card, Tag, Typography, Tooltip, Switch, Flex } from 'antd'
import { EditOutlined, ToolOutlined } from '@ant-design/icons'
import type { McpServer } from '@/api-client/types'
import { updateMcpServer, openMcpServerDrawer } from '../store'

const { Text } = Typography

interface McpServerCardProps {
  server: McpServer
  isEditable?: boolean
}

export function McpServerCard({
  server,
  isEditable = true,
}: McpServerCardProps) {
  const { message } = App.useApp()
  const [enableLoading, setEnableLoading] = useState(false)

  const handleEdit = () => {
    if (server.is_system) {
      openMcpServerDrawer(server, 'edit-system')
    } else {
      openMcpServerDrawer(server, 'edit')
    }
  }

  const handleToggleEnable = async (enabled: boolean) => {
    setEnableLoading(true)
    try {
      await updateMcpServer(server.id, {
        enabled,
      })
      message.success(`Server ${enabled ? 'enabled' : 'disabled'} successfully`)
    } catch (error) {
      console.error('Failed to toggle server enable state:', error)
      message.error(`Failed to ${enabled ? 'enable' : 'disable'} server`)
    } finally {
      setEnableLoading(false)
    }
  }

  return (
    <Card
      classNames={{
        body: '!p-3',
      }}
    >
      <div className="flex items-start gap-3 flex-wrap">
        {/* Server Info */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2 flex-wrap">
            <div className="flex-1 min-w-48">
              <Flex className="gap-2 items-center">
                <ToolOutlined aria-hidden="true" />
                <Text className="font-medium">{server.display_name}</Text>
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
                  >
                    Edit
                  </Button>
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
