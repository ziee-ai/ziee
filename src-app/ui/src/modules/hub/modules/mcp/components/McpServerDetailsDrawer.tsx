import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Typography, Card } from 'antd'
import { LinkOutlined } from '@ant-design/icons'
import type { HubMCPServer } from '@/api-client/types'

const { Title, Text } = Typography

interface McpServerDetailsDrawerProps {
  server: HubMCPServer | null
  open: boolean
  onClose: () => void
}

export function McpServerDetailsDrawer({
  server,
  open,
  onClose,
}: McpServerDetailsDrawerProps) {
  if (!server) return null

  return (
    <Drawer title={server.display_name} open={open} onClose={onClose}>
      <Flex vertical className="gap-4">
        {/* Basic Info */}
        <div>
          <Title level={3} className="!m-0 !mb-2">
            {server.display_name}
          </Title>
          <Text type="secondary" className="text-xs">
            {server.name}
          </Text>
          {server.description && (
            <div className="mt-2">
              <Text type="secondary">{server.description}</Text>
            </div>
          )}
        </div>

        {/* Command Information */}
        <div>
          <Title level={5}>Command</Title>
          <Card size="small" className="bg-gray-50">
            <Text code className="text-xs">
              {server.command} {server.args?.join(' ')}
            </Text>
          </Card>
        </div>

        {/* Environment Variables */}
        {server.environment_variables &&
          Object.keys(server.environment_variables).length > 0 && (
            <div>
              <Title level={5}>Environment Variables</Title>
              <Card size="small">
                <pre className="text-xs overflow-auto m-0">
                  {JSON.stringify(server.environment_variables, null, 2)}
                </pre>
              </Card>
            </div>
          )}

        {/* Server Details */}
        <div>
          <Title level={5}>Server Details</Title>
          <Flex vertical className="gap-2">
            {server.author && (
              <Flex justify="space-between">
                <Text type="secondary">Author:</Text>
                <Text>{server.author}</Text>
              </Flex>
            )}
            {server.popularity_score && (
              <Flex justify="space-between">
                <Text type="secondary">Popularity Score:</Text>
                <Text>{server.popularity_score}</Text>
              </Flex>
            )}
          </Flex>
        </div>

        {/* Links */}
        {(server.repository_url || server.homepage) && (
          <div>
            <Title level={5}>Links</Title>
            <Flex vertical className="gap-2">
              {server.repository_url && (
                <a
                  href={server.repository_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2"
                >
                  <LinkOutlined /> Repository
                </a>
              )}
              {server.homepage && (
                <a
                  href={server.homepage}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2"
                >
                  <LinkOutlined /> Homepage
                </a>
              )}
            </Flex>
          </div>
        )}

        {/* Tags */}
        {server.tags && server.tags.length > 0 && (
          <div>
            <Title level={5}>Tags</Title>
            <Flex wrap className="gap-1">
              {server.tags.map(tag => (
                <Tag key={tag} color="default">
                  {tag}
                </Tag>
              ))}
            </Flex>
          </div>
        )}
      </Flex>
    </Drawer>
  )
}
