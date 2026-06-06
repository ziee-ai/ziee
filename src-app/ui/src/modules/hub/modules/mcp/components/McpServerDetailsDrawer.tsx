import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, List, Tag, Typography, Card } from 'antd'
import {
  LinkOutlined,
  LockOutlined,
  KeyOutlined,
} from '@ant-design/icons'
import type { HubMCPServer, HubRequiredInput } from '@/api-client/types'

const { Title, Text } = Typography

/// Per-entry kind discriminator used only inside the drawer to tag
/// rows of the merged required-config list (env vars vs headers).
type RequiredInputRow = HubRequiredInput & { kind: 'env' | 'header' }

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

  // Merged required-config list: env vars + headers in one rendering
  // pass, each row tagged with its target surface so the user
  // understands whether they'll edit the env map or the headers map
  // post-install.
  const requiredInputs: RequiredInputRow[] = [
    ...(server.required_env ?? []).map(
      (v): RequiredInputRow => ({ ...v, kind: 'env' }),
    ),
    ...(server.required_headers ?? []).map(
      (v): RequiredInputRow => ({ ...v, kind: 'header' }),
    ),
  ]

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

        {/* Connection — shape depends on transport. stdio servers
            launch a local subprocess (show the resolved command line);
            http/sse servers connect to a remote URL (show as a
            clickable link). Anything else (or missing transport on
            legacy manifests) falls through to the stdio-style render
            since the install helper defaults to stdio. */}
        {server.transport_type === 'http' ||
        server.transport_type === 'sse' ||
        server.transport_type === 'streamable-http' ? (
          <div>
            <Title level={5}>URL</Title>
            <Card size="small" className="bg-gray-50">
              {server.url ? (
                <a
                  href={server.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 text-xs break-all"
                >
                  <LinkOutlined /> {server.url}
                </a>
              ) : (
                <Text type="secondary" className="text-xs">
                  No URL specified in manifest.
                </Text>
              )}
            </Card>
          </div>
        ) : (
          <div>
            <Title level={5}>Command</Title>
            <Card size="small" className="bg-gray-50">
              {server.command ? (
                <Text code className="text-xs break-all">
                  {server.command} {server.args?.join(' ')}
                </Text>
              ) : (
                <Text type="secondary" className="text-xs">
                  No command specified in manifest.
                </Text>
              )}
            </Card>
          </div>
        )}

        {/* Required configuration — primary view for env vars +
            headers that the user must set post-install. Each row
            tags the target surface (env var vs header) so the user
            knows where to edit. */}
        {requiredInputs.length > 0 && (
          <div>
            <Title level={5}>Required configuration</Title>
            <List
              size="small"
              dataSource={requiredInputs}
              renderItem={v => (
                <List.Item key={`${v.kind}:${v.name}`}>
                  <List.Item.Meta
                    avatar={
                      v.is_secret ? (
                        <LockOutlined className="text-orange-500" />
                      ) : (
                        <KeyOutlined className="text-blue-500" />
                      )
                    }
                    title={
                      <Flex className="gap-2 items-center" wrap>
                        <Text code className="text-xs">
                          {v.name}
                        </Text>
                        <Tag
                          color={v.kind === 'env' ? 'blue' : 'cyan'}
                          className="text-xs"
                        >
                          {v.kind === 'env' ? 'env var' : 'header'}
                        </Tag>
                        {v.is_secret && (
                          <Tag color="orange" className="text-xs">
                            secret
                          </Tag>
                        )}
                      </Flex>
                    }
                    description={
                      <Flex vertical className="gap-1">
                        {v.description && (
                          <Text type="secondary" className="text-xs">
                            {v.description}
                          </Text>
                        )}
                        {v.placeholder && (
                          <Text type="secondary" className="text-xs">
                            Example:{' '}
                            <Text code className="text-xs">
                              {v.placeholder}
                            </Text>
                          </Text>
                        )}
                        {v.docs_url && (
                          <a
                            href={v.docs_url}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-xs inline-flex items-center gap-1"
                          >
                            <LinkOutlined /> Get one
                          </a>
                        )}
                      </Flex>
                    }
                  />
                </List.Item>
              )}
            />
          </div>
        )}

        {/* Fallback — legacy catalog versions without `required_*`
            metadata still get the raw JSON dump so the user can
            inspect the env map. Suppressed when the structured view
            above is rendered (avoid duplicating info). */}
        {requiredInputs.length === 0 &&
          server.environment_variables &&
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
