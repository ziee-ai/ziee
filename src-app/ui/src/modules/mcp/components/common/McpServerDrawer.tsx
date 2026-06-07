import {
  Alert,
  Button,
  Form,
  Input,
  InputNumber,
  Select,
  Switch,
  App,
  Divider,
  Flex,
  Tooltip,
} from 'antd'
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
  type EnvVarEntry,
  type HeaderEntry,
} from '@/api-client/types'
import { KeyValueSecretEditor } from '@/modules/mcp/components/common/KeyValueSecretEditor'

const { TextArea } = Input

/// Form-state row shape for env vars and HTTP headers in this drawer.
/// `_was_saved_secret` is a hidden field set by the form initializer
/// when the entry came back from the server as a write-only secret
/// (is_secret=true, value=null) — controls the password-input
/// placeholder. Stripped from API payloads at submit time.
type EditorRow = {
  key: string
  value?: string
  is_secret: boolean
  _was_saved_secret?: boolean
}

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

// Mirrors the backend HOST_ALLOWED_COMMANDS (mcp/client/stdio.rs) and
// KNOWN_FLAVORS (code_sandbox/types.rs). The system-server form fetches
// the live lists from GET /code-sandbox/flavors; these are the fallback
// shown before that resolves. The backend re-validates on save.
const FALLBACK_HOST_COMMANDS = ['npx', 'uvx', 'python', 'python3', 'node']
const FALLBACK_FLAVOR_OPTIONS = [
  { value: 'full', label: 'full — Node + uv + python3 + R (~850 MB)' },
  { value: 'minimal', label: 'minimal — python3 only (~57 MB)' },
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

  // Sandbox flavor picker data, lazily fetched in system mode only (the
  // endpoint is admin-gated; non-admins never reach create/edit-system,
  // so this never fires for them — keeps the no-403 fixture happy).
  const [flavorOptions, setFlavorOptions] = useState(FALLBACK_FLAVOR_OPTIONS)
  const [hostCommands, setHostCommands] = useState<string[]>(
    FALLBACK_HOST_COMMANDS,
  )

  // OAuth is configurable only for user-owned HTTP servers (the endpoints are
  // owner-scoped). Built-in/system servers authenticate differently.
  const isUserMode = mode === 'create' || mode === 'edit'
  const isSystemMode = mode === 'create-system' || mode === 'edit-system'

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
            oauth_enabled: !!cfg,
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

  // Lazily load the sandbox flavor catalog + host command allowlist.
  // Only in system mode (admin-gated endpoint); falls back to the
  // hardcoded constants on error so the form still works offline.
  useEffect(() => {
    let cancelled = false
    if (open && isSystemMode) {
      Stores.McpServer.getSandboxFlavors()
        .then(resp => {
          if (cancelled) return
          if (resp.available.length > 0) {
            setFlavorOptions(
              resp.available.map(f => ({
                value: f.flavor,
                label: `${f.flavor} — ${f.description} (~${f.approximate_size_mb} MB)`,
              })),
            )
          }
          if (resp.host_allowed_commands.length > 0) {
            setHostCommands(resp.host_allowed_commands)
          }
        })
        .catch(() => {
          // keep fallbacks
        })
    }
    return () => {
      cancelled = true
    }
  }, [open, isSystemMode])

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
        // Structured entries from the server (write-only-secret
        // semantics — secret values come back as `value: null`). The
        // hidden `_was_saved_secret` field tells the password input
        // to render the `••••• (saved)` placeholder. On submit, rows
        // with `_was_saved_secret && value` empty get translated to
        // `value: null` in the API payload so the server preserves
        // the encrypted value.
        environment_variables_entries: (
          editingServer.environment_variables_entries ?? []
        ).map((entry): EditorRow => ({
          key: entry.key,
          value: entry.value ?? '',
          is_secret: entry.is_secret,
          _was_saved_secret: entry.is_secret && entry.value == null,
        })),
        headers_entries: (
          editingServer.headers_entries ?? []
        ).map((entry): EditorRow => ({
          key: entry.key,
          value: entry.value ?? '',
          is_secret: entry.is_secret,
          _was_saved_secret: entry.is_secret && entry.value == null,
        })),
        enabled: editingServer.enabled,
        supports_sampling: editingServer.supports_sampling ?? false,
        usage_mode: editingServer.usage_mode ?? 'auto',
        max_concurrent_sessions: editingServer.max_concurrent_sessions ?? undefined,
        run_in_sandbox: editingServer.run_in_sandbox ?? false,
        sandbox_flavor: editingServer.sandbox_flavor ?? 'full',
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
        sandbox_flavor: 'full',
      })
    }
  }, [editingServer, open, mode, form])

  // Parse the JSON-string `args` field (still a TextArea — it's a
  // flat array, no per-entry secret concept). Env vars + headers
  // come from Form.List as structured `EditorRow[]` and don't need
  // any JSON parsing here.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const parseArgsField = (values: any): string[] | null => {
    if (!values.args || !values.args.trim()) return []
    try {
      const parsed = JSON.parse(values.args)
      if (!Array.isArray(parsed)) {
        message.error('Arguments must be a JSON array')
        return null
      }
      return parsed
    } catch (_error) {
      message.error('Invalid JSON in arguments')
      return null
    }
  }

  /// Convert an `EditorRow[]` from the form to the API's
  /// `EnvVarEntry[]` / `HeaderEntry[]` shape. Strips the hidden
  /// `_was_saved_secret` field. For rows where the user left a
  /// saved-secret blank (didn't re-type the value), send
  /// `value: null` so the server keeps the existing encrypted
  /// value — without this the server would treat blank as an
  /// explicit empty-string and clobber the stored secret.
  const editorRowsToEntries = <T extends EnvVarEntry | HeaderEntry>(
    rows: EditorRow[] | undefined,
  ): T[] => {
    return (rows ?? [])
      .filter(row => row && row.key)
      .map(row => {
        const keepExistingSecret =
          row._was_saved_secret &&
          row.is_secret &&
          (row.value == null || row.value === '')
        return {
          key: row.key,
          value: keepExistingSecret ? null : (row.value ?? ''),
          is_secret: !!row.is_secret,
        } as unknown as T
      })
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

    const args = parseArgsField(values)
    if (args === null) return null
    const environment_variables_entries = editorRowsToEntries<EnvVarEntry>(
      values.environment_variables_entries,
    )
    const headers_entries = editorRowsToEntries<HeaderEntry>(
      values.headers_entries,
    )

    const serverData = {
      name: values.name,
      display_name: values.display_name,
      description: values.description,
      transport_type: values.transport_type,
      url: values.url,
      command: values.command,
      args: args,
      environment_variables_entries,
      headers_entries,
      enabled: values.enabled ?? true,
      supports_sampling: values.supports_sampling ?? false,
      usage_mode: values.usage_mode ?? 'auto',
      max_concurrent_sessions: values.max_concurrent_sessions ?? null,
      // Backend ignores `run_in_sandbox` for user-mode + non-stdio servers; we
      // still send it so the field round-trips through create + edit unchanged
      // when the toggle is visible.
      run_in_sandbox: values.run_in_sandbox ?? false,
      sandbox_flavor: values.sandbox_flavor ?? 'full',
      timeout_seconds: values.timeout_seconds ?? 30,
    }

    const updateData: UpdateMcpServerRequest = {
      display_name: values.display_name,
      description: values.description,
      url: values.url,
      command: values.command,
      args: args,
      environment_variables_entries,
      headers_entries,
      enabled: values.enabled ?? true,
      supports_sampling: values.supports_sampling ?? false,
      usage_mode: values.usage_mode ?? 'auto',
      max_concurrent_sessions: values.max_concurrent_sessions ?? null,
      // Backend ignores `run_in_sandbox` for user-mode + non-stdio servers; we
      // still send it so the field round-trips through create + edit unchanged
      // when the toggle is visible.
      run_in_sandbox: values.run_in_sandbox ?? false,
      sandbox_flavor: values.sandbox_flavor ?? 'full',
      timeout_seconds: values.timeout_seconds ?? 30,
    }

    let saved: McpServer
    if (mode === 'create') {
      const wrapped = await Stores.McpServer.createMcpServer(
        serverData as CreateMcpServerRequest,
      )
      saved = wrapped.server
      if (wrapped.connection_warning) {
        // Backend auto-downgraded enabled to false because the
        // connection probe failed. Surface the reason + 8s duration
        // so the user has time to read.
        message.warning({
          content: `MCP server saved but auto-disabled — ${wrapped.connection_warning.reason}`,
          duration: 8,
        })
      } else {
        message.success('MCP server created successfully')
      }
    } else if (mode === 'edit' && editingServer) {
      saved = await Stores.McpServer.updateMcpServer(editingServer.id, updateData)
      message.success('MCP server updated successfully')
    } else if (mode === 'create-system') {
      const wrapped = await Stores.SystemMcpServer.createSystemServer(
        serverData as CreateMcpServerRequest,
      )
      saved = wrapped.server
      if (wrapped.connection_warning) {
        message.warning({
          content: `System MCP server saved but auto-disabled — ${wrapped.connection_warning.reason}`,
          duration: 8,
        })
      } else {
        message.success('System MCP server created successfully')
      }
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
      const oauthEnabled = !!values.oauth_enabled
      const clientId = (values.oauth_client_id ?? '').trim()
      const clientSecret = values.oauth_client_secret ?? ''
      const scopes = (values.oauth_scopes ?? '').trim() || null
      try {
        if (!oauthEnabled) {
          // Section toggled off — clear any existing config. No-op if
          // there was nothing stored to begin with.
          if (hasExistingOAuth) {
            await Stores.McpServer.deleteMcpServerOAuthConfig(saved.id)
          }
        } else if (clientId && clientSecret) {
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
          // Cleared the client id (with section still enabled) →
          // remove the stored config.
          await Stores.McpServer.deleteMcpServerOAuthConfig(saved.id)
        }
        // (oauthEnabled + clientId set + secret blank + config exists
        //  → keep the current secret)
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

      // After persist, the SAVED server's entries are authoritative
      // (env_vars_entries has redacted secret values where applicable
      // — that's fine, the test handler falls back to the stored
      // decrypted value via `id`). For non-secret entries, send the
      // plaintext value directly.
      const payload: TestMcpConnectionRequest = {
        transport_type: saved.transport_type,
        command: saved.command ?? undefined,
        args: Array.isArray(saved.args) ? saved.args : [],
        environment_variables_entries:
          saved.environment_variables_entries ?? [],
        url: saved.url ?? undefined,
        headers_entries: saved.headers_entries ?? [],
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
      // Surface the backend's actual message (e.g. the
      // `MCP_ENABLE_FAILED_HEALTH_CHECK` probe failure reason)
      // instead of a generic toast. Use 8s duration so the user has
      // time to read what went wrong with their enable attempt.
      const reason =
        error instanceof Error && error.message
          ? error.message
          : 'Unknown error'
      message.error({
        content: `Failed to save MCP server: ${reason}`,
        duration: 8,
      })
      // If the failure is the enable-time health check, the backend
      // already persisted the OTHER fields and reverted enabled to
      // false. Re-fetch the server so the drawer's Enabled toggle
      // reflects the actual persisted state (not the user's
      // optimistic ON). Also reload the parent list so the row's
      // disabled badge updates.
      if (
        error instanceof Error &&
        error.message &&
        error.message.includes('MCP_ENABLE_FAILED_HEALTH_CHECK')
      ) {
        if (mode === 'edit' && editingServer) {
          try {
            const fresh = await Stores.McpServer.getMcpServer(editingServer.id)
            form.setFieldsValue({ enabled: fresh.enabled })
            setEnabledValue(!!fresh.enabled)
            Stores.McpServerDrawer.openMcpServerDrawer(fresh, 'edit')
          } catch (e) {
            console.warn('Failed to refresh server after health check:', e)
          }
        } else if (mode === 'edit-system' && editingServer) {
          try {
            const fresh = await Stores.SystemMcpServer.getSystemServerById(
              editingServer.id,
            )
            if (fresh) {
              form.setFieldsValue({ enabled: fresh.enabled })
            setEnabledValue(!!fresh.enabled)
              Stores.McpServerDrawer.openMcpServerDrawer(fresh, 'edit-system')
            }
          } catch (e) {
            console.warn('Failed to refresh system server after health check:', e)
          }
        }
      }
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
  const runInSandbox = Form.useWatch('run_in_sandbox', form)
  // A stdio server runs sandboxed (any command allowed) only when it's a
  // system server with the toggle on; otherwise it runs on the host and
  // its command must be in the host allowlist.
  const isSandboxed =
    isSystemMode && transportType === 'stdio' && runInSandbox === true
  // Local mirror for the title's Enabled Switch + a "currently
  // toggling" flag that disables the Switch (with a loading
  // spinner) while a save+probe round-trip is in flight.
  // Form.useWatch from OUTSIDE the <Form> provider tree was flaky;
  // local state + sync-from-form on open is the robust shape.
  const [enabledValue, setEnabledValue] = useState(false)
  const [togglingEnable, setTogglingEnable] = useState(false)

  // Sync the local mirror with the form's `enabled` field whenever
  // the drawer opens / switches mode / loads a new server. The form
  // is the source of truth at save time; this just keeps the title
  // Switch's checked prop in lockstep.
  useEffect(() => {
    if (!open) return
    if (mode === 'edit' || mode === 'edit-system') {
      setEnabledValue(!!editingServer?.enabled)
    } else {
      // Create mode — default to enabled=true (matches the form
      // initializer at line ~172).
      setEnabledValue(true)
    }
  }, [open, mode, editingServer])

  // Timeout is rendered at the END of each transport-specific block
  // (after env vars for stdio, after headers for http/sse — before
  // OAuth). Same Form.Item name so form state binds regardless of
  // which branch renders. Only one branch is mounted at a time
  // (gated on transportType), so the duplication is purely JSX
  // shape, not state.
  const timeoutField = (
    <Form.Item
      label="Timeout (seconds)"
      name="timeout_seconds"
      help="Maximum time to wait for a tool call response. Increase for servers that use sampling (multiple LLM calls)."
    >
      <InputNumber
        min={1}
        max={600}
        placeholder="30"
        style={{ width: '100%' }}
      />
    </Form.Item>
  )

  // Click handler for the title's Enabled Switch — drives the
  // server's enable lifecycle directly from the title (the bottom
  // Save button still works for editing other fields without
  // touching enable).
  //
  // Behaviors per direction:
  //   ON  — save the full current form state (so any in-flight env
  //         var / header / URL edits land) + force enabled=true;
  //         backend probes the persisted state. Probe success →
  //         server stays enabled. Probe failure → 400 with the
  //         reason, server reverts to enabled=false, Switch reverts.
  //   OFF — minimal PUT with just `enabled: false`. Does NOT save
  //         any other in-flight form edits (user explicit choice
  //         per the design discussion). No probe runs.
  //
  // Create mode: the Switch only updates the local form state; the
  // bottom Create button is what actually persists. Auto-saving on
  // a half-filled create form would surface validation errors out
  // of context.
  const handleEnabledToggle = async (v: boolean) => {
    if (mode === 'create' || mode === 'create-system') {
      setEnabledValue(v)
      form.setFieldsValue({ enabled: v })
      return
    }
    if (!editingServer) return

    setTogglingEnable(true)
    try {
      if (v === false) {
        // Minimal PUT — only the enabled flag, leave everything
        // else alone. Other in-flight form edits stay in the form
        // and are picked up by the next bottom-Save action.
        const payload: UpdateMcpServerRequest = { enabled: false }
        const updated =
          mode === 'edit'
            ? await Stores.McpServer.updateMcpServer(editingServer.id, payload)
            : await Stores.SystemMcpServer.updateSystemServer(
                editingServer.id,
                payload,
              )
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        Stores.McpServerDrawer.openMcpServerDrawer(updated, mode)
        message.success('Server disabled')
        return
      }

      // ON path — save the full current form (including the new
      // enabled=true) so the backend probes against the user's
      // intended config, not the stale persisted state.
      form.setFieldsValue({ enabled: true })
      setEnabledValue(true)
      try {
        const saved = await persistServer()
        if (!saved) {
          // Form validation failed — antd surfaced the field errors.
          // Revert the optimistic switch since nothing was persisted.
          setEnabledValue(false)
          form.setFieldsValue({ enabled: false })
          return
        }
        Stores.McpServerDrawer.openMcpServerDrawer(saved, mode)
        message.success('Server enabled — connection test passed')
      } catch (error) {
        // Most likely cause: MCP_ENABLE_FAILED_HEALTH_CHECK from
        // the probe (other fields persisted; enabled reverted to
        // false on the server). Surface the reason verbatim, then
        // refresh from the backend so the local mirror reflects
        // the persisted state.
        const reason =
          error instanceof Error && error.message
            ? error.message
            : 'Unknown error'
        message.error({
          content: `Failed to enable: ${reason}`,
          duration: 8,
        })
        try {
          const fresh =
            mode === 'edit'
              ? await Stores.McpServer.getMcpServer(editingServer.id)
              : Stores.SystemMcpServer.getSystemServerById(editingServer.id)
          if (fresh) {
            setEnabledValue(!!fresh.enabled)
            form.setFieldsValue({ enabled: fresh.enabled })
            Stores.McpServerDrawer.openMcpServerDrawer(fresh, mode)
          } else {
            setEnabledValue(false)
            form.setFieldsValue({ enabled: false })
          }
        } catch {
          setEnabledValue(false)
          form.setFieldsValue({ enabled: false })
        }
      }
    } finally {
      setTogglingEnable(false)
    }
  }

  // Title with the server-Enabled toggle on the right — keeps the
  // on/off control visible no matter how far the user scrolls.
  // Disabled in read-only mode (no edit permission). The Tooltip
  // also surfaces the persisted last-health-check info so the user
  // knows WHY the server is in its current state without having to
  // click Test Connection again.
  const healthAt = editingServer?.last_health_check_at
  const healthStatus = editingServer?.last_health_check_status
  const healthReason = editingServer?.last_health_check_reason
  const formatHealthTooltip = () => {
    const baseline = enabledValue
      ? 'Enabled — the server is reachable and queried by the LLM. Click to disable.'
      : 'Disabled — the server is not started or queried. Click to enable (a connection test will run first).'
    if (!healthAt || healthStatus === 'untested') {
      return baseline
    }
    const when = new Date(healthAt).toLocaleString()
    if (healthStatus === 'healthy') {
      return `${baseline}\n\nLast connection test: passed at ${when}`
    }
    return `${baseline}\n\nLast connection test failed at ${when}: ${
      healthReason ?? 'unknown reason'
    }`
  }
  const titleNode = (
    <Flex justify="space-between" align="center" className="w-full pr-6">
      <span>{getTitle()}</span>
      {!!editingServer || mode === 'create' || mode === 'create-system' ? (
        <Tooltip
          title={
            <span style={{ whiteSpace: 'pre-line' }}>
              {formatHealthTooltip()}
            </span>
          }
        >
          <Switch
            checked={enabledValue}
            loading={togglingEnable}
            disabled={!canManage || togglingEnable}
            onChange={handleEnabledToggle}
            checkedChildren="Enabled"
            unCheckedChildren="Disabled"
          />
        </Tooltip>
      ) : null}
    </Flex>
  )

  return (
    <Drawer open={open} onClose={handleClose} title={titleNode} size={600}>
      <div className="flex flex-col gap-4">
        {/* Surface the last probe's failure reason at the top of
            the body as an Alert so it can't be missed. Previously
            tucked into the title-Switch tooltip; that hid the
            reason behind a hover the user might never trigger.
            Renders only on unhealthy + only in edit mode (create
            mode has no probe history yet). */}
        {(mode === 'edit' || mode === 'edit-system') &&
          editingServer?.last_health_check_status === 'unhealthy' && (
            <Alert
              type="error"
              showIcon
              message={
                editingServer.last_health_check_at
                  ? `Connection test failed at ${new Date(editingServer.last_health_check_at).toLocaleString()}`
                  : 'Connection test failed'
              }
              description={
                editingServer.last_health_check_reason ??
                'No reason recorded.'
              }
            />
          )}
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
                dependencies={['run_in_sandbox', 'transport_type']}
                extra={
                  isSandboxed
                    ? 'Runs in the sandbox — any command is allowed.'
                    : `Allowed on host: ${hostCommands.join(', ')}. Enable “Run in sandbox” to use any command.`
                }
                rules={[
                  { required: true, message: 'Please enter a command' },
                  () => ({
                    validator(_, value) {
                      // Read run_in_sandbox from the form (not a captured
                      // closure) so re-validation triggered by toggling the
                      // switch sees the just-updated value synchronously.
                      const sandboxed =
                        isSystemMode &&
                        form.getFieldValue('transport_type') === 'stdio' &&
                        form.getFieldValue('run_in_sandbox') === true
                      if (sandboxed || !value) return Promise.resolve()
                      const base = String(value).trim().split(/\s+/)[0]
                      if (hostCommands.includes(base)) return Promise.resolve()
                      return Promise.reject(
                        new Error(
                          `Command '${base}' is not allowed on the host. Allowed: ${hostCommands.join(
                            ', ',
                          )}. Enable “Run in sandbox” to use any command.`,
                        ),
                      )
                    },
                  }),
                ]}
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
                help="One row per variable. Toggle 🔒 to encrypt at rest; secret values are never returned to the client after save (leave blank to keep)."
              >
                <KeyValueSecretEditor
                  name="environment_variables_entries"
                  defaultIsSecret={true}
                  keyPlaceholder="GITHUB_TOKEN"
                  valuePlaceholder="value"
                  labelSingular="env var"
                />
              </Form.Item>

              {timeoutField}
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
                help="One row per header. Toggle 🔒 to encrypt at rest (recommended for tokens / API keys). `${VAR}` interpolation against env vars is supported in header values."
              >
                <KeyValueSecretEditor
                  name="headers_entries"
                  defaultIsSecret={false}
                  keyPlaceholder="Authorization"
                  valuePlaceholder="Bearer …"
                  labelSingular="header"
                />
              </Form.Item>

              {timeoutField}

              {transportType === 'http' && isUserMode && (
                <>
                  <Divider className="text-sm text-gray-400 !mt-8">
                    OAuth 2.1
                  </Divider>
                  {/* Section-level enable toggle at the TOP of the
                      OAuth block. When off, the client id / secret /
                      scopes fields disappear entirely — keeps the
                      drawer compact for the common case (no OAuth)
                      and makes "turn off OAuth on this server"
                      explicit (saving with this off clears the
                      stored OAuth config). */}
                  <Form.Item
                    label="Enable OAuth 2.1"
                    name="oauth_enabled"
                    valuePropName="checked"
                    help="For servers requiring OAuth client_credentials. Turning this off on save clears any stored OAuth config."
                  >
                    <Switch />
                  </Form.Item>
                  <Form.Item
                    noStyle
                    shouldUpdate={(prev, curr) =>
                      prev.oauth_enabled !== curr.oauth_enabled
                    }
                  >
                    {({ getFieldValue }) =>
                      getFieldValue('oauth_enabled') ? (
                        <>
                          <Form.Item
                            label="OAuth Client ID"
                            name="oauth_client_id"
                            help="Client ID issued by the upstream OAuth server."
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
                      ) : null
                    }
                  </Form.Item>
                </>
              )}
            </>
          )}

          {/* `Enabled` lives in the drawer title now (always visible
              regardless of scroll). `Timeout` lives at the end of
              each transport-specific block. Sampling stays here as
              its own section because it's transport-agnostic. */}
          <Divider className="text-sm text-gray-400 !mt-8">Sampling</Divider>

          {/* Supports Sampling */}
          <Form.Item
            label="Enable MCP Sampling"
            name="supports_sampling"
            valuePropName="checked"
            help="Allow this server to request LLM completions inline during tool execution (requires HTTP transport and server support)"
          >
            <Switch />
          </Form.Item>

          {/* Sampling sub-fields — only meaningful when sampling is
              enabled; hide entirely otherwise to reduce noise. */}
          <Form.Item
            noStyle
            shouldUpdate={(prev, curr) =>
              prev.supports_sampling !== curr.supports_sampling
            }
          >
            {({ getFieldValue }) =>
              getFieldValue('supports_sampling') ? (
                <>
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

                  <Form.Item
                    label="Max Concurrent Sessions"
                    name="max_concurrent_sessions"
                    help="Limit simultaneous sampling sessions. Leave blank for unlimited. Users over the limit receive a friendly error."
                  >
                    <InputNumber min={1} placeholder="Unlimited" style={{ width: '100%' }} />
                  </Form.Item>
                </>
              ) : null
            }
          </Form.Item>

          {/* Run in sandbox (system + stdio only) */}
          {transportType === 'stdio' &&
            (mode === 'create-system' || mode === 'edit-system') && (
              <>
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
                      servers will not see your real files. First use
                      downloads the selected sandbox image.
                    </>
                  }
                >
                  <Switch
                    onChange={() => {
                      // Re-validate the command field: turning the toggle
                      // on lifts the host allowlist; turning it off
                      // re-imposes it.
                      form.validateFields(['command']).catch(() => {})
                    }}
                  />
                </Form.Item>

                {runInSandbox && (
                  <Form.Item
                    label="Sandbox flavor"
                    name="sandbox_flavor"
                    help="Rootfs image the sandboxed server runs in. 'full' ships Node (npx), uv (uvx), python3 and R; 'minimal' is python3-only."
                  >
                    <Select options={flavorOptions} />
                  </Form.Item>
                )}
              </>
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
