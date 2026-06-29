import { useState } from 'react'
import { Alert, Button, Card, Confirm, Tag, Text, Tooltip, Switch, Flex } from '@/components/ui'
import { Pencil, Wrench, Trash2, Plug } from 'lucide-react'
import { message } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type McpServer,
  type TestMcpConnectionRequest,
} from '@/api-client/types'

// System and user MCP servers gate on different permission namespaces.
// `server.is_system` selects which set applies at render time. `test` maps to
// the create-level permission because the test-connection endpoint requires it.
const SYSTEM_PERMS = {
  edit: Permissions.McpServersAdminEdit,
  delete: Permissions.McpServersAdminDelete,
  test: Permissions.McpServersAdminCreate,
} as const
const USER_PERMS = {
  edit: Permissions.McpServersEdit,
  delete: Permissions.McpServersDelete,
  test: Permissions.McpServersCreate,
} as const

interface McpServerCardProps {
  server: McpServer
  isEditable?: boolean
  bordered?: boolean
}

export function McpServerCard({
  server,
  isEditable = true,
}: McpServerCardProps) {
  const [enableLoading, setEnableLoading] = useState(false)
  const [testing, setTesting] = useState(false)

  const perms = server.is_system ? SYSTEM_PERMS : USER_PERMS
  const canEdit = usePermission(perms.edit)
  const canDelete = usePermission(perms.delete)
  const canTest = usePermission(perms.test)

  const handleEdit = () => {
    if (server.is_system) {
      Stores.McpServerDrawer.openMcpServerDrawer(server, 'edit-system')
    } else {
      Stores.McpServerDrawer.openMcpServerDrawer(server, 'edit')
    }
  }

  const handleDelete = async () => {
    try {
      if (server.is_system) {
        await Stores.SystemMcpServer.deleteSystemServer(server.id)
      } else {
        await Stores.McpServer.deleteMcpServer(server.id)
      }
      message.success('Server deleted successfully')
    } catch (_error) {
      message.error('Failed to delete server')
    }
  }

  const handleTest = async () => {
    setTesting(true)
    try {
      // Probe the persisted config. The OAuth secret is write-only, so we send
      // the server `id` and let the backend reuse the stored secret (URL matches).
      // Probe the persisted config. Secret values are write-only in
      // the response (env_vars_entries / headers_entries carry
      // is_secret + value=null) — the test handler falls back to the
      // stored decrypted value via `id`.
      const payload: TestMcpConnectionRequest = {
        transport_type: server.transport_type,
        command: server.command ?? undefined,
        args: Array.isArray(server.args) ? server.args : [],
        environment_variables_entries:
          server.environment_variables_entries ?? [],
        url: server.url ?? undefined,
        headers_entries: server.headers_entries ?? [],
        timeout_seconds: server.timeout_seconds,
        id: server.id,
      }
      const result = server.is_system
        ? await Stores.SystemMcpServer.testSystemServerConnection(payload)
        : await Stores.McpServer.testMcpServerConnection(payload)
      if (result.success) {
        message.success(result.message || 'Connection successful')
      } else {
        message.error(result.message || 'Connection failed')
      }
      // Backend recorded the probe outcome into the row's
      // `last_health_check_*` columns. Refresh the parent list so
      // this card's `server` prop re-renders with the updated
      // health tag (Healthy/Unhealthy) without requiring the user
      // to reload the page.
      try {
        if (server.is_system) {
          await Stores.SystemMcpServer.loadSystemServers()
        } else {
          await Stores.McpServer.loadMcpServers()
        }
      } catch (e) {
        console.warn('Failed to refresh after Test Connection:', e)
      }
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Connection test failed')
    } finally {
      setTesting(false)
    }
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

  return (
    <Card
      data-testid={`mcp-server-card-${server.id}`}
    >
      <div className="flex items-start gap-3 flex-wrap">
        {/* Server Info */}
        <div className="flex-1">
          {/* Header row — transport type is already conveyed by the
            * Tag inside; the per-transport background band has been
            * removed so MCP cards match the visual rhythm of the
            * other settings cards (Model Repositories, Rootfs
            * environments). */}
          <div className="mb-3 flex items-center gap-2 flex-wrap">
            <div className="flex-1 min-w-48">
              <Flex className="gap-2 items-center">
                <Wrench aria-hidden="true" className="text-base" />
                <Text className="font-semibold text-base">{server.display_name}</Text>
                {!isEditable && server.is_system && (
                  <Tag tone="info" data-testid="mcp-server-system-tag">System</Tag>
                )}
                <Tag
                  data-testid="mcp-server-transport-tag"
                  tone={
                    server.transport_type === 'stdio'
                      ? 'info'
                      : server.transport_type === 'http'
                        ? 'success'
                        : 'info'
                  }
                >
                  {server.transport_type.toUpperCase()}
                </Tag>
                {server.supports_sampling && (
                  <Tooltip title={`Sampling enabled · ${server.usage_mode === 'always' ? 'Always mode' : 'Auto mode'}`}>
                    <Tag tone="info" data-testid="mcp-sampling-badge">Sampling</Tag>
                  </Tooltip>
                )}
                {server.usage_mode === 'always' && (
                  <Tag tone="warning" data-testid="mcp-always-badge">Always</Tag>
                )}
                {/* Health status from the last probe — surfaces
                    boot-time auto-disable reasons + Test Connection
                    results + enable-time probe failures. Always
                    renders SOMETHING (incl. "Untested") so the
                    user can confirm at a glance whether the server
                    has been probed and what the outcome was. */}
                {(() => {
                  const status =
                    server.last_health_check_status ?? 'untested'
                  if (status === 'unhealthy') {
                    return (
                      <Tooltip
                        title={
                          <span style={{ whiteSpace: 'pre-line' }}>
                            {`Last connection test failed${
                              server.last_health_check_at
                                ? ` at ${new Date(server.last_health_check_at).toLocaleString()}`
                                : ''
                            }${
                              server.last_health_check_reason
                                ? `:\n${server.last_health_check_reason}`
                                : ''
                            }`}
                          </span>
                        }
                      >
                        <Tag tone="error" data-testid="mcp-health-unhealthy">
                          Unhealthy
                        </Tag>
                      </Tooltip>
                    )
                  }
                  if (status === 'healthy') {
                    return (
                      <Tooltip
                        title={`Last connection test passed${
                          server.last_health_check_at
                            ? ` at ${new Date(server.last_health_check_at).toLocaleString()}`
                            : ''
                        }`}
                      >
                        <Tag tone="success" data-testid="mcp-health-healthy">
                          Healthy
                        </Tag>
                      </Tooltip>
                    )
                  }
                  return (
                    <Tooltip title="Connection has not been tested yet. Click Test Connection or toggle Enabled to run a probe.">
                      <Tag data-testid="mcp-health-untested">Untested</Tag>
                    </Tooltip>
                  )
                })()}
              </Flex>
            </div>
            <div className="flex gap-2 items-center justify-end">
              {isEditable && (
                <>
                  {canEdit && (
                    <Tooltip
                      title={server.enabled ? 'Disable Server' : 'Enable Server'}
                    >
                      <Switch
                        checked={server.enabled}
                        onChange={handleToggleEnable}
                        loading={enableLoading}
                        aria-label={`${server.enabled ? 'Disable' : 'Enable'} ${server.display_name}`}
                        data-testid="mcp-server-enable-switch"
                      />
                    </Tooltip>
                  )}
                  {canTest && (
                    <Tooltip title="Test the connection to this server">
                      <Button
                        icon={<Plug />}
                        loading={testing}
                        onClick={e => {
                          e.stopPropagation()
                          handleTest()
                        }}
                        aria-label={`Test connection to ${server.display_name}`}
                        data-testid="mcp-server-test-btn"
                      >
                        Test
                      </Button>
                    </Tooltip>
                  )}
                  {canEdit && (
                    <Button
                      icon={<Pencil />}
                      onClick={e => {
                        e.stopPropagation()
                        handleEdit()
                      }}
                      data-testid="mcp-server-edit-btn"
                    >
                      Edit
                    </Button>
                  )}
                  {canDelete && !server.is_built_in && (
                    <Confirm
                      title="Delete Server"
                      description={`Are you sure you want to delete "${server.display_name}"? This action cannot be undone.`}
                      okText="Delete"
                      cancelText="Cancel"
                      okButtonProps={{ danger: true }}
                      onConfirm={handleDelete}
                      data-testid="mcp-server-delete-confirm"
                    >
                      <Button
                        icon={<Trash2 />}
                        variant="destructive"
                        onClick={e => {
                          e.stopPropagation()
                          if (server.enabled) {
                            message.warning(
                              'Please disable the server before deleting it',
                            )
                          }
                        }}
                        aria-label={`Delete ${server.display_name}`}
                        data-testid="mcp-server-delete-btn"
                      >
                        Delete
                      </Button>
                    </Confirm>
                  )}
                </>
              )}
            </div>
          </div>

          {/* Surface the last probe's failure reason inline as an
              Alert so it can't be missed (previously hidden in a
              Tooltip on the tag). Renders only for the unhealthy
              case; the Healthy / Untested tags carry their own
              tooltip with sufficient detail. */}
          {server.last_health_check_status === 'unhealthy' && (
            <Alert
              tone="error"
              className="!mb-2"
              data-testid="mcp-server-health-alert"
              title={
                server.last_health_check_at
                  ? `Connection test failed at ${new Date(server.last_health_check_at).toLocaleString()}`
                  : 'Connection test failed'
              }
              description={
                server.last_health_check_reason ?? 'No reason recorded.'
              }
            />
          )}

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
                <Card size="sm" className={'!mt-2'} data-testid="mcp-server-command-card">
                  <pre className="text-xs overflow-auto m-0">
                    {server.command}
                    {Array.isArray(server.args) &&
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
