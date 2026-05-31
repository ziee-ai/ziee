import { Button, Form, Input, InputNumber, Select, Switch, App, Divider } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useMcpServerDrawerStore } from '@/modules/mcp/stores'
import {
  showConnectionTestResult,
  showConnectionTestError,
} from '@/modules/mcp/components/common/connectionTestToast'
import {
  Permissions,
  type CreateMcpServerRequest,
  type UpdateMcpServerRequest,
  type TestMcpConnectionRequest,
  type McpServer,
} from '@/api-client/types'

const { TextArea } = Input

const TRANSPORT_TYPES = [
  {
    label: 'Standard I/O',
    value: 'stdio',
    description:
      'Start MCP server as a local process communicating via stdin/stdout',
  },
  {
    label: 'HTTP',
    value: 'http',
    description: 'Connect to MCP server via HTTP/HTTPS endpoint',
  },
  {
    label: 'Server-Sent Events',
    value: 'sse',
    description: 'Connect to MCP server via Server-Sent Events',
  },
]

export function McpServerDrawer() {
  const [form] = Form.useForm()
  const { message } = App.useApp()

  const { open, loading, mode, editingServer } = useMcpServerDrawerStore()

  // Whether the server being edited already has a stored OAuth config — used to
  // decide between keep/replace/remove on save and to label the secret field.
  const [hasExistingOAuth, setHasExistingOAuth] = useState(false)

  // Local loading for the "Save & Test Connection" action (save, then probe).
  const [testing, setTesting] = useState(false)

  // OAuth is configurable only for user-owned HTTP servers (the endpoints are
  // owner-scoped). Built-in/system servers authenticate differently.
  const isUserMode = mode === 'create' || mode === 'edit'

  // Mirror the user/ + assistants/ pattern (audit I-3): gate the form
  // by mode-specific manage permissions so the drawer becomes read-
  // only if a perm is revoked while it's open.
  const canCreateUser = usePermission(Permissions.McpServersCreate)
  const canEditUser = usePermission(Permissions.McpServersEdit)
  const canCreateSystem = usePermission(Permissions.McpServersAdminCreate)
  const canEditSystem = usePermission(Permissions.McpServersAdminEdit)
  const canManage = (() => {
    switch (mode) {
      case 'create':
        return canCreateUser
      case 'edit':
        return canEditUser
      case 'create-system':
        return canCreateSystem
      case 'edit-system':
        return canEditSystem
      default:
        return false
    }
  })()

  // Load any existing OAuth config when editing a user HTTP server.
  useEffect(() => {
    let cancelled = false
    if (
      mode === 'edit' &&
      editingServer &&
      open &&
      editingServer.transport_type === 'http'
    ) {
      Stores.McpServer.getMcpServerOAuthConfig(editingServer.id)
        .then(cfg => {
          if (cancelled) return
          setHasExistingOAuth(!!cfg)
          form.setFieldsValue({
            oauth_client_id: cfg?.client_id ?? '',
            oauth_client_secret: '',
            oauth_scopes: cfg?.scopes ?? '',
          })
        })
        .catch(() => {
          if (!cancelled) setHasExistingOAuth(false)
        })
    } else {
      setHasExistingOAuth(false)
    }
    return () => {
      cancelled = true
    }
  }, [editingServer, open, mode, form])

  // Populate form when editing server changes
  useEffect(() => {
    if (editingServer && open && (mode === 'edit' || mode === 'edit-system')) {
      const formValues = {
        name: editingServer.name,
        display_name: editingServer.display_name,
        description: editingServer.description,
        transport_type: editingServer.transport_type,
        url: editingServer.url,
        command: editingServer.command,
        args:
          editingServer.args && editingServer.args.length > 0
            ? JSON.stringify(editingServer.args, null, 2)
            : '',
        env: editingServer.environment_variables
          ? JSON.stringify(editingServer.environment_variables, null, 2)
          : '',
        headers:
          editingServer.headers &&
          Object.keys(editingServer.headers).length > 0
            ? JSON.stringify(editingServer.headers, null, 2)
            : '',
        enabled: editingServer.enabled,
        supports_sampling: editingServer.supports_sampling ?? false,
        usage_mode: editingServer.usage_mode ?? 'auto',
        max_concurrent_sessions: editingServer.max_concurrent_sessions ?? undefined,
        run_in_sandbox: editingServer.run_in_sandbox ?? false,
        timeout_seconds: editingServer.timeout_seconds ?? 30,
      }
      form.setFieldsValue(formValues)
    } else if (open && (mode === 'create' || mode === 'create-system')) {
      form.resetFields()
      form.setFieldsValue({
        transport_type: 'stdio',
        enabled: true,
        supports_sampling: false,
        usage_mode: 'auto',
      })
    }
  }, [editingServer, open, mode, form])

  // Parse the JSON-string transport fields (args / env / headers) shared by
  // save + test. Returns null (after surfacing a message) on malformed input.
  type ParsedTransport = {
    args: string[]
    environmentVariables: Record<string, string>
    headers: Record<string, string>
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const parseTransportFields = (values: any): ParsedTransport | null => {
    let args: string[] = []
    if (values.args && values.args.trim()) {
      try {
        const parsed = JSON.parse(values.args)
        if (!Array.isArray(parsed)) {
          message.error('Arguments must be a JSON array')
          return null
        }
        args = parsed
      } catch (_error) {
        message.error('Invalid JSON in arguments')
        return null
      }
    }

    let environmentVariables: Record<string, string> = {}
    if (values.env && values.env.trim()) {
      try {
        const parsed = JSON.parse(values.env)
        if (typeof parsed !== 'object' || Array.isArray(parsed)) {
          message.error('Environment variables must be a JSON object')
          return null
        }
        environmentVariables = parsed
      } catch (_error) {
        message.error('Invalid JSON in environment variables')
        return null
      }
    }

    let headers: Record<string, string> = {}
    if (values.headers && values.headers.trim()) {
      try {
        const parsed = JSON.parse(values.headers)
        if (typeof parsed !== 'object' || Array.isArray(parsed)) {
          message.error('HTTP Headers must be a JSON object')
          return null
        }
        headers = parsed
      } catch (_error) {
        message.error('Invalid JSON in HTTP Headers')
        return null
      }
    }

    return { args, environmentVariables, headers }
  }

  // Validate + persist the form (create or update) and return the saved server
  // (incl. its id / is_system / transport_type) so callers can act on it.
  // Returns null when the form is invalid (antd surfaces the field errors) or a
  // handled precondition fails (e.g. an OAuth client id without a secret).
  // A create/update API failure is thrown so the caller can report it.
  const persistServer = async (): Promise<McpServer | null> => {
    let values
    try {
      values = await form.validateFields()
    } catch {
      // antd already highlights the invalid fields — nothing more to surface.
      return null
    }

    const parsed = parseTransportFields(values)
    if (!parsed) return null
    const { args, environmentVariables, headers } = parsed

    const serverData = {
      name: values.name,
      display_name: values.display_name,
      description: values.description,
      transport_type: values.transport_type,
      url: values.url,
      command: values.command,
      args: args,
      environment_variables: environmentVariables,
      headers: headers,
      enabled: values.enabled ?? true,
      supports_sampling: values.supports_sampling ?? false,
      usage_mode: values.usage_mode ?? 'auto',
      max_concurrent_sessions: values.max_concurrent_sessions ?? null,
      // Backend ignores `run_in_sandbox` for user-mode + non-stdio servers; we
      // still send it so the field round-trips through create + edit unchanged
      // when the toggle is visible.
      run_in_sandbox: values.run_in_sandbox ?? false,
      timeout_seconds: values.timeout_seconds ?? 30,
    }

    const updateData: UpdateMcpServerRequest = {
      display_name: values.display_name,
      description: values.description,
      url: values.url,
      command: values.command,
      args: args,
      environment_variables: environmentVariables,
      headers: headers,
      enabled: values.enabled ?? true,
      supports_sampling: values.supports_sampling ?? false,
      usage_mode: values.usage_mode ?? 'auto',
      max_concurrent_sessions: values.max_concurrent_sessions ?? null,
      // Backend ignores `run_in_sandbox` for user-mode + non-stdio servers; we
      // still send it so the field round-trips through create + edit unchanged
      // when the toggle is visible.
      run_in_sandbox: values.run_in_sandbox ?? false,
      timeout_seconds: values.timeout_seconds ?? 30,
    }

    let saved: McpServer
    if (mode === 'create') {
      saved = await Stores.McpServer.createMcpServer(
        serverData as CreateMcpServerRequest,
      )
      message.success('MCP server created successfully')
    } else if (mode === 'edit' && editingServer) {
      saved = await Stores.McpServer.updateMcpServer(editingServer.id, updateData)
      message.success('MCP server updated successfully')
    } else if (mode === 'create-system') {
      saved = await Stores.SystemMcpServer.createSystemServer(
        serverData as CreateMcpServerRequest,
      )
      message.success('System MCP server created successfully')
    } else if (mode === 'edit-system' && editingServer) {
      saved = await Stores.SystemMcpServer.updateSystemServer(
        editingServer.id,
        updateData,
      )
      message.success('System MCP server updated successfully')
    } else {
      // Unreachable guard for the unused 'clone'/default modes (mirrors
      // canManage's `default: false`); no save is attempted.
      return null
    }

    // Once a fresh server exists, any outcome that leaves the drawer OPEN must
    // rebind it to edit mode so the NEXT action updates it instead of creating a
    // duplicate. Used by the post-create OAuth failure paths below (the
    // secret-missing early return and OAuth API errors). The plain-Save success
    // path closes the drawer and never calls this; Save&Test does its own flip.
    const flipToEditIfFreshCreate = () => {
      if (mode === 'create' || mode === 'create-system') {
        Stores.McpServerDrawer.openMcpServerDrawer(
          saved,
          saved.is_system ? 'edit-system' : 'edit',
        )
      }
    }

    // Persist OAuth config for user-owned HTTP servers.
    if (isUserMode && values.transport_type === 'http') {
      const clientId = (values.oauth_client_id ?? '').trim()
      const clientSecret = values.oauth_client_secret ?? ''
      const scopes = (values.oauth_scopes ?? '').trim() || null
      try {
        if (clientId && clientSecret) {
          await Stores.McpServer.setMcpServerOAuthConfig(saved.id, {
            client_id: clientId,
            client_secret: clientSecret,
            scopes,
          })
        } else if (clientId && !clientSecret && !hasExistingOAuth) {
          message.error('Enter the OAuth client secret to enable OAuth')
          flipToEditIfFreshCreate()
          return null
        } else if (!clientId && hasExistingOAuth) {
          // Cleared the client id → remove the stored config.
          await Stores.McpServer.deleteMcpServerOAuthConfig(saved.id)
        }
        // (clientId set, secret blank, config exists → keep the current secret)
      } catch (error) {
        // The server was already created/updated; rebind a fresh create to edit
        // before the error propagates so a retry can't create a duplicate.
        flipToEditIfFreshCreate()
        throw error
      }
    }

    return saved
  }

  // "Save & Test Connection": persist the entered settings first, then probe the
  // PERSISTED server (by id, so the backend reuses any stored OAuth secret —
  // same as the card). On a fresh create we flip the drawer to edit mode so a
  // second click updates rather than creating a duplicate. The drawer stays open
  // so the test result and saved state remain visible.
  const handleSaveAndTest = async () => {
    setTesting(true)
    try {
      const saved = await persistServer()
      if (!saved) return

      if (mode === 'create' || mode === 'create-system') {
        Stores.McpServerDrawer.openMcpServerDrawer(
          saved,
          saved.is_system ? 'edit-system' : 'edit',
        )
      }

      const payload: TestMcpConnectionRequest = {
        transport_type: saved.transport_type,
        command: saved.command ?? undefined,
        args: Array.isArray(saved.args) ? saved.args : [],
        environment_variables: saved.environment_variables ?? {},
        url: saved.url ?? undefined,
        headers: saved.headers ?? {},
        timeout_seconds: saved.timeout_seconds,
        id: saved.id,
      }
      const result = saved.is_system
        ? await Stores.SystemMcpServer.testSystemServerConnection(payload)
        : await Stores.McpServer.testMcpServerConnection(payload)
      showConnectionTestResult(message, result)
    } catch (error) {
      showConnectionTestError(message, error)
    } finally {
      setTesting(false)
    }
  }

  const handleSubmit = async () => {
    try {
      Stores.McpServerDrawer.setMcpServerDrawerLoading(true)
      const saved = await persistServer()
      if (!saved) return
      Stores.McpServerDrawer.closeMcpServerDrawer()
      form.resetFields()
    } catch (error) {
      console.error('Failed to save MCP server:', error)
      message.error('Failed to save MCP server')
    } finally {
      Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
    }
  }

  const handleClose = () => {
    Stores.McpServerDrawer.closeMcpServerDrawer()
    form.resetFields()
  }

  const getTitle = () => {
    switch (mode) {
      case 'create':
        return 'Add MCP Server'
      case 'edit':
        return 'Edit MCP Server'
      case 'create-system':
        return 'Add System Server'
      case 'edit-system':
        return 'Edit System Server'
      default:
        return 'MCP Server'
    }
  }

  const getButtonText = () => {
    switch (mode) {
      case 'create':
      case 'create-system':
        return 'Create'
      case 'edit':
      case 'edit-system':
        return 'Save'
      default:
        return 'Save'
    }
  }

  const transportType = Form.useWatch('transport_type', form)

  return (
    <Drawer open={open} onClose={handleClose} title={getTitle()} size={600}>
      <div className="flex flex-col gap-4">
        <Form
          name="mcp-server-form"
          form={form}
          layout="vertical"
          disabled={!canManage}
        >
          {/* Name (only for create mode) */}
          {(mode === 'create' || mode === 'create-system') && (
            <Form.Item
              label="Name"
              name="name"
              rules={[
                { required: true, message: 'Please enter a name' },
                {
                  pattern: /^[a-z0-9-]+$/,
                  message:
                    'Name must contain only lowercase letters, numbers, and hyphens',
                },
              ]}
            >
              <Input placeholder="e.g., filesystem, fetch, custom-tool" />
            </Form.Item>
          )}

          {/* Display Name */}
          <Form.Item
            label="Display Name"
            name="display_name"
            rules={[{ required: true, message: 'Please enter a display name' }]}
          >
            <Input placeholder="e.g., Filesystem Access, Web Fetch" />
          </Form.Item>

          {/* Description */}
          <Form.Item label="Description" name="description">
            <TextArea
              placeholder="Brief description of what this server does"
              rows={2}
            />
          </Form.Item>

          {/* Transport Type */}
          <Form.Item
            label="Transport Type"
            name="transport_type"
            rules={[
              { required: true, message: 'Please select a transport type' },
            ]}
          >
            <Select
              disabled={mode === 'edit' || mode === 'edit-system'}
              options={TRANSPORT_TYPES.map(type => ({
                ...type,
                disabled:
                  (mode === 'edit' || mode === 'edit-system') && editingServer
                    ? editingServer.transport_type !== type.value
                    : false,
              }))}
            />
          </Form.Item>

          {/* Transport-specific fields */}
          {transportType === 'stdio' && (
            <>
              <Form.Item
                label="Command"
                name="command"
                rules={[{ required: true, message: 'Please enter a command' }]}
              >
                <Input placeholder="e.g., npx, uvx, node" />
              </Form.Item>

              <Form.Item
                label="Arguments"
                name="args"
                help="JSON array format, e.g., [&quot;-y&quot;, &quot;@modelcontextprotocol/server-filesystem&quot;]"
              >
                <TextArea
                  placeholder='["-y", "@modelcontextprotocol/server-filesystem"]'
                  rows={3}
                  className="font-mono text-xs"
                />
              </Form.Item>

              <Form.Item
                label="Environment Variables"
                name="env"
                help="JSON object format, e.g., {&quot;KEY&quot;: &quot;value&quot;}"
              >
                <TextArea
                  placeholder='{"KEY": "value"}'
                  rows={4}
                  className="font-mono text-xs"
                />
              </Form.Item>
            </>
          )}

          {(transportType === 'http' || transportType === 'sse') && (
            <>
              <Form.Item
                label="URL"
                name="url"
                rules={[
                  { required: true, message: 'Please enter a URL' },
                  { type: 'url', message: 'Please enter a valid URL' },
                ]}
              >
                <Input placeholder="https://example.com/mcp" />
              </Form.Item>

              <Form.Item
                label="HTTP Headers"
                name="headers"
                help={'JSON object format, e.g., {"Authorization": "Bearer token"}'}
              >
                <TextArea
                  placeholder={'{"Authorization": "Bearer token"}'}
                  rows={4}
                  className="font-mono text-xs"
                />
              </Form.Item>

              {transportType === 'http' && isUserMode && (
                <>
                  <Divider className="text-sm text-gray-400">
                    OAuth 2.1
                  </Divider>
                  <Form.Item
                    label="OAuth Client ID"
                    name="oauth_client_id"
                    help="For servers requiring OAuth 2.1 (client_credentials). Leave blank for none; clear to remove."
                  >
                    <Input placeholder="client id" autoComplete="off" />
                  </Form.Item>
                  <Form.Item
                    label="OAuth Client Secret"
                    name="oauth_client_secret"
                    help={
                      hasExistingOAuth
                        ? 'A secret is stored. Leave blank to keep it; enter a value to replace it.'
                        : 'Stored securely and never shown again.'
                    }
                  >
                    <Input.Password
                      placeholder={hasExistingOAuth ? '•••••••• (unchanged)' : 'client secret'}
                      autoComplete="new-password"
                    />
                  </Form.Item>
                  <Form.Item
                    label="OAuth Scopes"
                    name="oauth_scopes"
                    help="Optional, space-separated (e.g. 'mcp read')."
                  >
                    <Input placeholder="mcp" autoComplete="off" />
                  </Form.Item>
                </>
              )}
            </>
          )}

          {/* Enabled */}
          <Form.Item label="Enabled" name="enabled" valuePropName="checked">
            <Switch />
          </Form.Item>

          {/* Timeout */}
          <Form.Item
            label="Timeout (seconds)"
            name="timeout_seconds"
            help="Maximum time to wait for a tool call response. Increase for servers that use sampling (multiple LLM calls)."
          >
            <InputNumber min={1} max={600} placeholder="30" style={{ width: '100%' }} />
          </Form.Item>

          <Divider className="text-sm text-gray-400">Sampling</Divider>

          {/* Supports Sampling */}
          <Form.Item
            label="Enable MCP Sampling"
            name="supports_sampling"
            valuePropName="checked"
            help="Allow this server to request LLM completions inline during tool execution (requires HTTP transport and server support)"
          >
            <Switch />
          </Form.Item>

          {/* Usage Mode */}
          <Form.Item
            label="Usage Mode"
            name="usage_mode"
            help="Auto: LLM decides when to call this server. Always: server is called before every LLM request to enrich context."
          >
            <Select
              options={[
                { label: 'Auto (LLM decides)', value: 'auto' },
                { label: 'Always (pre-process every prompt)', value: 'always' },
              ]}
            />
          </Form.Item>

          {/* Max Concurrent Sessions */}
          <Form.Item
            label="Max Concurrent Sessions"
            name="max_concurrent_sessions"
            help="Limit simultaneous sampling sessions. Leave blank for unlimited. Users over the limit receive a friendly error."
          >
            <InputNumber min={1} placeholder="Unlimited" style={{ width: '100%' }} />
          </Form.Item>

          {/* Run in sandbox (system + stdio only) */}
          {transportType === 'stdio' &&
            (mode === 'create-system' || mode === 'edit-system') && (
              <Form.Item
                label="Run in sandbox"
                name="run_in_sandbox"
                valuePropName="checked"
                help={
                  <>
                    Launch this stdio MCP server inside the code_sandbox
                    bwrap isolation. On Linux runs natively; on macOS /
                    Windows it routes through a microVM. The server only
                    sees an isolated workspace — filesystem-oriented MCP
                    servers will not see your real files. First use may
                    download a small sandbox image (~57 MB).
                  </>
                }
              >
                <Switch />
              </Form.Item>
            )}
        </Form>

        <div className="flex gap-2 justify-end">
          {canManage && !!transportType && (
            <Button
              className="mr-auto"
              loading={testing}
              disabled={loading}
              onClick={handleSaveAndTest}
            >
              Save &amp; Test Connection
            </Button>
          )}
          <Button onClick={handleClose}>
            {canManage ? 'Cancel' : 'Close'}
          </Button>
          {canManage && (
            <Button
              type="primary"
              loading={loading}
              disabled={testing}
              onClick={handleSubmit}
            >
              {getButtonText()}
            </Button>
          )}
        </div>
      </div>
    </Drawer>
  )
}
