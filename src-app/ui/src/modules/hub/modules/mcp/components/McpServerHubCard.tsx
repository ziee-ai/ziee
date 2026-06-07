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
import {
  Permissions,
  type HubMCPServer,
  type TransportType,
} from '@/api-client/types'
import { useState } from 'react'
import { McpServerDetailsDrawer } from '@/modules/hub/modules/mcp/components/McpServerDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useNavigate } from 'react-router-dom'
import type { McpServerDrawerPrefill } from '@/modules/mcp/stores/McpServerDrawer.store'

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

  /**
   * Translate a hub MCP manifest into the McpServerDrawer's prefill
   * shape so the drawer opens fully populated with the catalog's
   * defaults. The user reviews, fills in any required secrets, then
   * submits via the normal /mcp/servers POST — the backend records
   * the install in `hub_entities` via the `hub_id` field we
   * forward through the request body.
   *
   * Marks every `required_env` / `required_header` entry as a
   * secret with `value: ''` so the form renders the redacted input
   * with a "(required)" hint instead of pre-filling a placeholder.
   */
  const prefillFromHub = (): McpServerDrawerPrefill => ({
    fields: {
      name: server.name,
      display_name: server.display_name,
      description: server.description,
      transport_type: (server.transport_type ?? 'stdio') as TransportType,
      command: server.command,
      args: server.args,
      url: server.url,
      environment_variables_entries: [
        // Hub-supplied defaults (free-form key/value map in the
        // manifest). Treated as non-secret because the catalog
        // wouldn't ship a real secret in plaintext.
        ...Object.entries(
          (server.environment_variables ?? {}) as Record<string, string>,
        ).map(([key, value]) => ({
          key,
          value: String(value ?? ''),
          is_secret: false,
        })),
        // Required-secret env vars the user must fill in. Tagged
        // as secret so the form renders a redacted input.
        ...(server.required_env ?? []).map(e => ({
          key: e.name,
          value: '',
          is_secret: true,
        })),
      ],
      headers_entries: [
        ...Object.entries(
          (server.headers ?? {}) as Record<string, string>,
        ).map(([key, value]) => ({
          key,
          value: String(value ?? ''),
          is_secret: false,
        })),
        ...(server.required_headers ?? []).map(e => ({
          key: e.name,
          value: '',
          is_secret: true,
        })),
      ],
      supports_sampling: server.supports_sampling ?? false,
      enabled: true,
    },
    hub_id: server.id,
  })

  /**
   * "Install for me" — opens the drawer in `create` (user-scope) mode
   * prefilled from the hub manifest. The drawer's save path POSTs
   * /api/mcp/servers with `hub_id` so the backend records the
   * install in `hub_entities`. Replaces the prior silent createFromHub
   * call: the user always reviews + fills in secrets before saving.
   */
  const handleInstall = () => {
    try {
      setInstalling(true)
      Stores.McpServerDrawer.openMcpServerDrawer(
        undefined,
        'create',
        prefillFromHub(),
      )
      if (requiresSetup) {
        message.info({
          content: `Review settings + configure ${requiredInputsLabel}, then save.`,
          duration: 5,
        })
      }
    } finally {
      // Drawer is mounted; the spinner clears immediately. The user
      // sees the drawer open, not a long loading state.
      setInstalling(false)
    }
  }

  /**
   * "Install for the system" — admin path. Opens the drawer in
   * `create-system` mode prefilled from the hub manifest. Same
   * mechanism: drawer save POSTs /api/mcp/system-servers with
   * `hub_id`.
   */
  const handleInstallAsSystem = () => {
    try {
      setInstallingSystem(true)
      Stores.McpServerDrawer.openMcpServerDrawer(
        undefined,
        'create-system',
        prefillFromHub(),
      )
      if (requiresSetup) {
        message.info({
          content: `Review settings + configure ${requiredInputsLabel}, then save.`,
          duration: 5,
        })
      }
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
                  {/* Always render a transport tag so users can tell
                      at a glance whether the server runs locally
                      (stdio) or talks to a remote URL (http/sse).
                      Missing `transport_type` in the manifest is
                      treated as stdio per the install helper. */}
                  <Tag className="text-xs">
                    {(server.transport_type ?? 'stdio').toUpperCase()}
                  </Tag>
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
                {/* Install button layout — permission-based:
                    * Admin (canInstallSystem) → TWO buttons:
                      "Install for me" + "Install for the system".
                      Both open the McpServerDrawer prefilled from
                      the hub manifest; the user reviews + fills in
                      secrets, then submits via the regular create
                      endpoint with `hub_id` forwarded.
                    * Non-admin → ONE button "Install" (user-scope).
                    Same behavior on web and desktop (where the
                    single user is admin and gets both buttons). */}
                {isAlreadyInstalled && !canInstallSystem ? (
                  <Button
                    icon={<EyeOutlined />}
                    onClick={e => {
                      e.stopPropagation()
                      navigate('/settings/mcp-servers')
                    }}
                  >
                    View Server
                  </Button>
                ) : canInstallSystem ? (
                  <>
                    {canInstall && (
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
                        Install for me
                      </Button>
                    )}
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
                        : 'Install for the system'}
                    </Button>
                  </>
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
