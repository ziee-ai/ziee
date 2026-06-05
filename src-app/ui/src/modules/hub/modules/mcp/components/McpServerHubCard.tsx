import { App, Card, Tag, Tooltip, Typography, Button, Flex } from 'antd'
import {
  DownloadOutlined,
  StarOutlined,
  GlobalOutlined,
  GithubOutlined,
  EyeOutlined,
  CopyOutlined,
  KeyOutlined,
} from '@ant-design/icons'
import { Permissions, type HubMCPServer } from '@/api-client/types'
import { useState } from 'react'
import { McpServerDetailsDrawer } from '@/modules/hub/modules/mcp/components/McpServerDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
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
  const [installingSystem, setInstallingSystem] = useState(false)
  const canInstall = usePermission(Permissions.HubMcpServersCreate)
  const canInstallSystem = usePermission(Permissions.McpServersAdminCreate)

  // Check if server was already created from this hub server
  const isAlreadyInstalled = server.created_ids && server.created_ids.length > 0
  // Check if a SYSTEM-WIDE server already exists for this hub_id
  // (is_system=true, user_id=NULL). Backend rejects duplicates with
  // 409; the UI uses this to disable the button + show a clearer
  // "System Installed" label.
  const isAlreadyInstalledAsSystem =
    server.created_system_ids && server.created_system_ids.length > 0

  // Combined list of required inputs (env vars + headers) the user
  // must configure post-install. Treated as one list for card UX —
  // the structured per-type view lives in the details drawer.
  // Each entry carries a kind discriminator so the toast text can
  // disambiguate when a name appears on both surfaces (catalog
  // author mistake, but worth surfacing rather than rendering it
  // twice silently).
  const requiredInputs: { name: string; kind: 'env' | 'header' }[] = [
    ...(server.required_env ?? []).map(v => ({
      name: v.name,
      kind: 'env' as const,
    })),
    ...(server.required_headers ?? []).map(v => ({
      name: v.name,
      kind: 'header' as const,
    })),
  ]
  const requiresSetup = requiredInputs.length > 0
  const requiredInputsLabel = requiredInputs
    .map(i => (i.kind === 'header' ? `${i.name} (header)` : i.name))
    .join(', ')

  const handleInstall = async () => {
    try {
      setInstalling(true)

      // Create MCP server from hub via store action
      await Stores.HubMcpServers.createFromHub({
        hub_id: server.id,
        name: server.name,
        display_name: server.display_name,
        enabled: true,
      })

      if (requiresSetup) {
        // Use a longer-lived toast when the user has work to do —
        // 6s gives them time to register the list of keys before
        // the message disappears.
        message.success({
          content: `${server.display_name} installed. Configure ${requiredInputsLabel} in /settings/mcp-servers before using.`,
          duration: 6,
        })
      } else {
        message.success(`${server.display_name} installed successfully!`)
      }

      // Navigate to user MCP servers after creation
      navigate('/settings/mcp-servers')
    } catch (error: any) {
      console.error('Failed to install MCP server:', error)
      message.error(
        `Failed to install MCP server: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setInstalling(false)
    }
  }

  const handleInstallAsSystem = async () => {
    try {
      setInstallingSystem(true)
      // Install as a SYSTEM-WIDE MCP server (is_system=true, no
      // owner — enforced by the `system_server_must_have_no_owner`
      // CHECK constraint in migration 7). Visible to every user in
      // the system MCP server list; admins manage via the system
      // MCP admin page.
      await Stores.HubMcpServers.createSystemFromHub({
        hub_id: server.id,
        name: server.name,
        display_name: server.display_name,
        enabled: true,
      })

      if (requiresSetup) {
        message.success({
          content: `System MCP server "${server.display_name}" installed. Configure ${requiredInputsLabel} in /settings/mcp-admin before using.`,
          duration: 6,
        })
      } else {
        message.success(
          `System MCP server "${server.display_name}" installed.`,
        )
      }

      // Navigate to the system MCP admin page so the admin can see it.
      navigate('/settings/mcp-admin')
    } catch (error: any) {
      console.error('Failed to install system MCP server:', error)
      message.error(
        `Failed to install as system: ${error.message || 'Unknown error'}`,
      )
    } finally {
      setInstallingSystem(false)
    }
  }

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-server-id={server.id}
        data-testid={`hub-mcp-card-${server.id}`}
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
                  {isAlreadyInstalled && <Tag color="green">Installed</Tag>}
                  {isAlreadyInstalledAsSystem && (
                    <Tag color="purple">System installed</Tag>
                  )}
                  {requiresSetup && (
                    <Tooltip
                      title={`Requires setup after install: ${requiredInputsLabel}`}
                    >
                      <Tag
                        color="warning"
                        icon={<KeyOutlined />}
                        className="text-xs"
                        data-testid="hub-mcp-requires-setup-tag"
                      >
                        Requires setup
                      </Tag>
                    </Tooltip>
                  )}
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
                {isAlreadyInstalled ? (
                  <Button
                    icon={<EyeOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      navigate('/settings/mcp-servers')
                    }}
                  >
                    View Server
                  </Button>
                ) : canInstall ? (
                  <Button
                    type="primary"
                    icon={<DownloadOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleInstall()
                    }}
                    disabled={installing || installingSystem}
                    loading={installing}
                    data-testid="hub-mcp-install-btn"
                  >
                    Install
                  </Button>
                ) : null}
                {/* "Install as System" — admin power-user action.
                    Shown when the user holds BOTH permissions
                    (`hub::mcp_servers::create` AND
                    `mcp_servers_admin::create`) regardless of
                    whether the per-user "Installed" badge is set
                    (a personal install doesn't preclude also
                    installing as system). Default-styled +
                    distinct `CopyOutlined` icon so it's visually
                    separable from the primary "Install" action.
                    Disabled when a system install already exists
                    — backend rejects duplicates with 409, but
                    disabling here gives the admin clear feedback
                    without a round-trip. */}
                {canInstall && canInstallSystem && (
                  <Button
                    icon={<CopyOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      handleInstallAsSystem()
                    }}
                    loading={installingSystem}
                    disabled={
                      installing ||
                      installingSystem ||
                      isAlreadyInstalledAsSystem
                    }
                    data-testid="hub-mcp-install-as-system-btn"
                  >
                    {isAlreadyInstalledAsSystem
                      ? 'System Installed'
                      : 'Install as System'}
                  </Button>
                )}
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
                  <Flex
                    wrap
                    className="gap-1"
                    style={{ display: 'inline-flex' }}
                  >
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
