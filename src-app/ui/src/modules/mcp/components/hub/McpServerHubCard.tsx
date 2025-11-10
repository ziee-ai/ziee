import { App, Card, Tag, Typography, Button, Flex } from 'antd'
import {
  DownloadOutlined,
  StarOutlined,
  GlobalOutlined,
  GithubOutlined,
  EyeOutlined,
} from '@ant-design/icons'
import type { HubMCPServer, CreateMcpServerRequest } from '@/api-client/types'
import { useState } from 'react'
import { McpServerDetailsDrawer } from './McpServerDetailsDrawer'
import { Stores } from '@/core/stores'
import { useNavigate } from 'react-router-dom'

const { Text } = Typography

interface McpServerHubCardProps {
  server: HubMCPServer
}

export function McpServerHubCard({ server }: McpServerHubCardProps) {
  const { message } = App.useApp()
  const navigate = useNavigate()
  const [showDetails, setShowDetails] = useState(false)
  const [installing, setInstalling] = useState(false)

  const { systemServers } = Stores.SystemMcpServer

  // Check if server was already created from this hub server
  const existingServer = systemServers.find(
    (s: any) => s.source?.type === 'hub' && s.source?.id === server.id
  )

  const handleInstall = async () => {
    try {
      setInstalling(true)

      // Determine transport type and required fields
      const transport_type = server.transport_type || 'stdio'

      if (transport_type === 'stdio' && !server.command) {
        message.error('Cannot install: stdio server missing command')
        setInstalling(false)
        return
      }

      const request: CreateMcpServerRequest = {
        name: server.name,
        display_name: server.display_name,
        description: server.description || undefined,
        transport_type: transport_type as any,
        command: server.command || '',
        args: server.args || [],
        environment_variables: server.environment_variables || undefined,
        enabled: true,
        source: { type: 'hub', id: server.id },
      }

      await Stores.SystemMcpServer.createSystemServer(request)
      message.success(`${server.display_name} installed successfully!`)

      // Navigate to /settings/mcp-admin after creation
      navigate('/settings/mcp-admin')
    } catch (error: any) {
      console.error('Failed to install MCP server:', error)
      message.error(
        `Failed to install MCP server: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setInstalling(false)
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-server-id={server.id}
      >
        <div className="flex items-start gap-3 flex-wrap">
          {/* Server Info */}
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center">
                  {server.icon_url && (
                    <img
                      src={server.icon_url}
                      alt={server.display_name}
                      className="w-6 h-6 rounded"
                    />
                  )}
                  <Text className="font-medium cursor-pointer">
                    {server.display_name}
                  </Text>
                  {server.category && (
                    <Tag color="blue" className="text-xs">
                      {server.category}
                    </Tag>
                  )}
                  {server.transport_type && (
                    <Tag className="text-xs">
                      {server.transport_type.toUpperCase()}
                    </Tag>
                  )}
                  {installing && <Tag color="blue">Installing...</Tag>}
                  {existingServer && <Tag color="green">Installed</Tag>}
                </Flex>
              </div>
              <div className="flex gap-1 items-center justify-end">
                {server.homepage && (
                  <Button
                    icon={<GlobalOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      window.open(server.homepage, '_blank')
                    }}
                  />
                )}
                {server.repository_url && (
                  <Button
                    icon={<GithubOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      window.open(server.repository_url, '_blank')
                    }}
                  />
                )}
                <Button
                  type={existingServer ? undefined : 'primary'}
                  icon={existingServer ? <EyeOutlined /> : <DownloadOutlined />}
                  onClick={e => {
                    e.stopPropagation()
                    if (existingServer) {
                      navigate('/settings/mcp-admin')
                    } else {
                      handleInstall()
                    }
                  }}
                  disabled={installing}
                  loading={installing}
                >
                  {existingServer ? 'View Server' : 'Install'}
                </Button>
              </div>
            </div>

            <div>
              {server.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {server.description}
                </Text>
              )}

              {/* Tags */}
              {server.tags && server.tags.length > 0 && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Tags:
                  </Text>
                  <Flex wrap className="gap-1" style={{ display: 'inline-flex' }}>
                    {server.tags.slice(0, 3).map(tag => (
                      <Tag key={tag} color="default" className="text-xs">
                        {tag}
                      </Tag>
                    ))}
                    {server.tags.length > 3 && (
                      <Tag color="default" className="text-xs">
                        +{server.tags.length - 3}
                      </Tag>
                    )}
                  </Flex>
                </div>
              )}

              {/* Metadata */}
              <div className="mb-2">
                <Flex wrap className="gap-4 text-xs">
                  {server.author && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Author:
                      </Text>{' '}
                      {server.author}
                    </span>
                  )}
                  {server.tool_count && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Tools:
                      </Text>{' '}
                      {server.tool_count}
                    </span>
                  )}
                  {server.download_count && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Downloads:
                      </Text>{' '}
                      {server.download_count.toLocaleString()}
                    </span>
                  )}
                  {server.rating && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Rating:
                      </Text>{' '}
                      <StarOutlined /> {server.rating.toFixed(1)}
                    </span>
                  )}
                </Flex>
              </div>
            </div>
          </div>
        </div>
      </Card>

      <McpServerDetailsDrawer
        server={showDetails ? server : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />
    </>
  )
}
