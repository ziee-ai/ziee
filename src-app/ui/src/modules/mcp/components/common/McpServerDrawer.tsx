import {
  Alert,
  Button,
  Form,
  FormField,
  useForm,
  Input,
  PasswordInput,
  Textarea,
  InputNumber,
  Select,
  Switch,
  Separator,
  Tabs,
  Tooltip,
  message,
  useWatch,
} from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { McpToolCallsTab } from '@/modules/mcp/components/common/McpToolCallsTab'
import { useEffect, useMemo, useState } from 'react'
import { usePermission } from '@/core/permissions'
import { type CreateMcpServerRequest, type UpdateMcpServerRequest, type TestMcpConnectionRequest, type McpServer, type EnvVarEntry, type HeaderEntry, type UsageMode, type TransportType } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { KeyValueSecretEditor } from '@/modules/mcp/components/common/KeyValueSecretEditor'
import { SystemMcpServer } from '@/modules/mcp/stores/systemMcpServer'
import { McpUserPolicy } from '@/modules/mcp/stores/mcpUserPolicy'
import { McpServer as McpServerStore } from '@/modules/mcp/stores/mcpServer'
import { McpServerDrawer as McpServerDrawerStore } from '@/modules/mcp/stores/mcpServerDrawer'
import { SandboxFlavors as SandboxFlavorsStore } from '@/modules/code-sandbox/stores/sandboxFlavors'

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

// The fallback values for flavor options + host commands now live
// in the shared SandboxFlavors store (code-sandbox/stores/) — see
// the `FALLBACK_OPTIONS` + `FALLBACK_HOST_COMMANDS` constants there.

export function McpServerDrawer() {
  const form = useForm({
    defaultValues: {
      name: '',
      display_name: '',
      description: '',
      transport_type: 'stdio',
      url: '',
      command: '',
      args: '',
      environment_variables_entries: [] as EditorRow[],
      headers_entries: [] as EditorRow[],
      enabled: true,
      supports_sampling: false,
      usage_mode: 'auto',
      max_concurrent_sessions: undefined as number | undefined,
      run_in_sandbox: false,
      sandbox_flavor: 'full',
      timeout_seconds: 30,
      oauth_enabled: false,
      oauth_client_id: '',
      oauth_client_secret: '',
      oauth_scopes: '',
    },
  })

  // Helper to set multiple fields at once (analog of antd's setFieldsValue).
  const setFieldsValue = (vals: Record<string, unknown>) => {
    for (const [k, v] of Object.entries(vals)) {
      form.setValue(k as Parameters<typeof form.setValue>[0], v as any)
    }
  }

  // Read the drawer state via the Stores proxy (not the raw zustand hook) so
  // render subscribes through the meta-framework's per-field proxy, matching
  // how the rest of this component drives the store (McpServerDrawerStore.*).
  const { open, loading, mode, editingServer, prefillData } =
    McpServerDrawerStore
  // Read the policy state property (not the function accessors) so
  // the React proxy installs a useStore subscription — without this
  // the drawer's transport dropdown + user-mode sandbox info Alert
  // would NOT re-render when the admin saves a new policy
  // (function-typed proxy properties don't subscribe; see
  // core/stores.ts:250-280).
  const { policy: userPolicy } = McpUserPolicy
  // Memoize so the derived array is reference-stable across renders
  // when `userPolicy` hasn't actually changed. Without this, the
  // useEffect below that depends on the array's stringified contents
  // would re-fire on every render while policy is null (each render
  // produces a fresh `[]`).
  const policyAllowedTransports = useMemo(
    () => userPolicy?.allowed_transports ?? [],
    [userPolicy],
  )
  const policyUserStdioSandboxFlavor =
    userPolicy?.user_stdio_sandbox_flavor ?? null
  // Set to true when a Hub-prefill carried a transport_type the
  // active policy disallows AND we auto-substituted the first
  // allowed transport. The form area uses this to surface an
  // inline Alert explaining the swap so the user isn't surprised
  // their stdio install opened as http.
  const [prefillTransportSwapped, setPrefillTransportSwapped] = useState<
    null | { from: string; to: string }
  >(null)
  // Whether the server being edited already has a stored OAuth config — used to
  // decide between keep/replace/remove on save and to label the secret field.
  const [hasExistingOAuth, setHasExistingOAuth] = useState(false)

  // Local loading for the "Save & Test Connection" action (save, then probe).
  const [testing, setTesting] = useState(false)

  // Sandbox flavor catalog + host command allowlist — shared via the
  // SandboxFlavors store (lazy-loaded on first access, cached for
  // the session). McpUserPolicyCard reads the same store. The
  // FALLBACK_* constants the store ships with cover the offline /
  // pre-load case so the form is usable before the fetch resolves.
  const { selectOptions: flavorOptions, hostCommands } =
    SandboxFlavorsStore

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
      McpServerStore.getMcpServerOAuthConfig(editingServer.id)
        .then(cfg => {
          if (cancelled) return
          setHasExistingOAuth(!!cfg)
          setFieldsValue({
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
  }, [editingServer, open, mode])

  // Sandbox flavors + host command allowlist are loaded by the
  // shared SandboxFlavors store (declared above) on first access.
  // No per-drawer fetch needed; both the system-mode flavor Select
  // and the host-tier command validator read from the store.

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
          Array.isArray(editingServer.args) && editingServer.args.length > 0
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
      form.reset(formValues as any)
    } else if (open && (mode === 'create' || mode === 'create-system')) {
      form.reset()
      // Base defaults — overridden below by prefillData (Hub-install flow).
      // Note: in user mode with `policy.allowed_transports = ['http']` only,
      // `'stdio'` would be rejected at submit. The transport Select is
      // filtered against the policy (see `visibleTransports`), so the
      // initial 'stdio' value disappears from the dropdown and the user is
      // forced to pick a permitted option.
      setFieldsValue({
        transport_type: 'stdio',
        enabled: true,
        supports_sampling: false,
        usage_mode: 'auto',
        sandbox_flavor: 'full',
      })
      if (prefillData?.fields) {
        // Hub-install flow: manifest values flow straight into the form
        // so the user reviews + fills in secrets before saving. Translate
        // server-shape fields back into the form's editor-row shape.
        const f = prefillData.fields

        // Policy-mismatch swap: in user mode (`create`), if the hub
        // manifest's transport isn't in the policy's allow-list, fall
        // back to the first allowed transport. Without this the user
        // would land in the drawer with a transport_type that's not
        // selectable (filtered out of the dropdown) and submit would
        // 422 — they'd have no obvious way to fix it. We surface the
        // swap with an inline Alert so they know we changed it.
        let effectiveTransport = f.transport_type
        if (
          mode === 'create' &&
          effectiveTransport &&
          policyAllowedTransports.length > 0 &&
          !policyAllowedTransports.includes(effectiveTransport)
        ) {
          const replacement = policyAllowedTransports[0]
          setPrefillTransportSwapped({
            from: String(effectiveTransport),
            to: replacement,
          })
          effectiveTransport = replacement as typeof effectiveTransport
        } else {
          setPrefillTransportSwapped(null)
        }

        setFieldsValue({
          name: f.name,
          display_name: f.display_name,
          description: f.description,
          transport_type: effectiveTransport,
          url: f.url,
          command: f.command,
          args:
            f.args && Array.isArray(f.args) && f.args.length > 0
              ? JSON.stringify(f.args, null, 2)
              : '',
          environment_variables_entries: (
            f.environment_variables_entries ?? []
          ).map((entry): EditorRow => ({
            key: entry.key,
            value: entry.value ?? '',
            is_secret: entry.is_secret,
          })),
          headers_entries: (f.headers_entries ?? []).map(
            (entry): EditorRow => ({
              key: entry.key,
              value: entry.value ?? '',
              is_secret: entry.is_secret,
            }),
          ),
          enabled: f.enabled ?? true,
          supports_sampling: f.supports_sampling ?? false,
          usage_mode: f.usage_mode ?? 'auto',
          timeout_seconds: f.timeout_seconds ?? 30,
        })
      } else {
        setPrefillTransportSwapped(null)
      }
    }
  }, [
    editingServer,
    open,
    mode,
    prefillData,
    // Re-run on policy change too — if the admin tightens the policy
    // while the drawer is mid-prefill (rare; mostly defensive).
    policyAllowedTransports.join(','),
  ])

  // Sandbox flavors are now loaded by the shared SandboxFlavors
  // store (declared up at the top). The store's __init__.flavors
  // hook fires on first store access, so we don't need a per-drawer
  // fetch effect here.

  // Parse the JSON-string `args` field (still a Textarea — it's a
  // flat array, no per-entry secret concept). Env vars + headers
  // come from FormList as structured `EditorRow[]` and don't need
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

  // Manual validation helper: sets a field error and marks the field
  // as touched so FormField renders it.
  const setFieldError = (field: string, msg: string) => {
    form.setError(field as any, { message: msg })
    form.setValue(field as any, form.getValues(field as any), {
      shouldTouch: true,
    })
  }

  // Validate + persist the form (create or update) and return the saved server
  // (incl. its id / is_system / transport_type) so callers can act on it.
  // Returns null when the form is invalid or a handled precondition fails.
  // A create/update API failure is thrown so the caller can report it.
  const persistServer = async (): Promise<McpServer | null> => {
    const values = form.getValues()

    // Conditional required-field validation (mirrors the antd rules).
    // Clear stale errors first so a retry doesn't see the previous run's errors.
    form.clearErrors()
    let hasError = false

    if ((mode === 'create' || mode === 'create-system') && !values.name?.trim()) {
      setFieldError('name', 'Please enter a name')
      hasError = true
    }
    if (
      (mode === 'create' || mode === 'create-system') &&
      values.name &&
      !/^[a-z0-9-]+$/.test(values.name)
    ) {
      setFieldError(
        'name',
        'Name must contain only lowercase letters, numbers, and hyphens',
      )
      hasError = true
    }
    if (!values.display_name?.trim()) {
      setFieldError('display_name', 'Please enter a display name')
      hasError = true
    }
    if (!values.transport_type) {
      setFieldError('transport_type', 'Please select a transport type')
      hasError = true
    }
    if (values.transport_type === 'stdio' && !values.command?.trim()) {
      setFieldError('command', 'Please enter a command')
      hasError = true
    } else if (values.transport_type === 'stdio' && values.command?.trim()) {
      // Host command allowlist (ignored when sandboxed).
      const sandboxed =
        isSystemMode && form.getValues('run_in_sandbox' as any) === true
      if (!sandboxed) {
        const base = String(values.command).trim().split(/\s+/)[0]
        if (!hostCommands.includes(base)) {
          setFieldError(
            'command',
            `Command '${base}' is not allowed on the host. Allowed: ${hostCommands.join(
              ', ',
            )}. Enable "Run in sandbox" to use any command.`,
          )
          hasError = true
        }
      }
    }
    if (
      (values.transport_type === 'http' || values.transport_type === 'sse') &&
      !values.url?.trim()
    ) {
      setFieldError('url', 'Please enter a URL')
      hasError = true
    }
    if (values.url?.trim() && !/^https?:\/\/.+/.test(values.url.trim())) {
      setFieldError('url', 'Please enter a valid URL')
      hasError = true
    }

    if (hasError) return null

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
      usage_mode: (values.usage_mode ?? 'auto') as UsageMode,
      max_concurrent_sessions: values.max_concurrent_sessions ?? undefined,
      // For user-mode stdio the backend force-overwrites both fields
      // from the active policy; the values we send here are ignored.
      // For system mode the admin's choices are honored verbatim.
      run_in_sandbox: values.run_in_sandbox ?? false,
      sandbox_flavor: values.sandbox_flavor ?? 'full',
      timeout_seconds: values.timeout_seconds ?? 30,
      // Hub-tracking pass-through: when the drawer was opened from a
      // Hub MCP card, `prefillData.hub_id` is set and we forward it
      // so the backend records the install in `hub_entities`.
      hub_id: prefillData?.hub_id ?? null,
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
      usage_mode: (values.usage_mode ?? 'auto') as UsageMode,
      max_concurrent_sessions: values.max_concurrent_sessions ?? undefined,
      // Same force-override semantics on update for user-mode stdio
      // (the policy re-applies on every save). System mode honors
      // the admin's choices.
      run_in_sandbox: values.run_in_sandbox ?? false,
      sandbox_flavor: values.sandbox_flavor ?? 'full',
      timeout_seconds: values.timeout_seconds ?? 30,
    }

    let saved: McpServer
    if (mode === 'create') {
      const wrapped = await McpServerStore.createMcpServer(
        serverData as CreateMcpServerRequest,
      )
      // Wrapper is flattened: McpServer fields at top level +
      // optional `connection_warning` sibling. Strip the warning to
      // get a plain McpServer for downstream consumers.
      const { connection_warning, ...row } = wrapped
      saved = row as McpServer
      if (connection_warning) {
        // Backend auto-downgraded enabled to false because the
        // connection probe failed. Surface the reason + 8s duration
        // so the user has time to read.
        message.warning(`MCP server saved but auto-disabled — ${connection_warning.reason}`, { duration: 8000 })
      } else {
        message.success('MCP server created successfully')
      }
    } else if (mode === 'edit' && editingServer) {
      saved = await McpServerStore.updateMcpServer(editingServer.id, updateData)
      message.success('MCP server updated successfully')
    } else if (mode === 'create-system') {
      const wrapped = await SystemMcpServer.createSystemServer(
        serverData as CreateMcpServerRequest,
      )
      const { connection_warning, ...row } = wrapped
      saved = row as McpServer
      if (connection_warning) {
        message.warning(`System MCP server saved but auto-disabled — ${connection_warning.reason}`, { duration: 8000 })
      } else {
        message.success('System MCP server created successfully')
      }
    } else if (mode === 'edit-system' && editingServer) {
      saved = await SystemMcpServer.updateSystemServer(
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
    // duplicate. Used by the post-create OAuth failure paths below.
    const flipToEditIfFreshCreate = () => {
      if (mode === 'create' || mode === 'create-system') {
        McpServerDrawerStore.openMcpServerDrawer(
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
      const scopes = (values.oauth_scopes ?? '').trim() || undefined
      try {
        if (!oauthEnabled) {
          // Section toggled off — clear any existing config.
          if (hasExistingOAuth) {
            await McpServerStore.deleteMcpServerOAuthConfig(saved.id)
          }
        } else if (clientId && clientSecret) {
          await McpServerStore.setMcpServerOAuthConfig(saved.id, {
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
          await McpServerStore.deleteMcpServerOAuthConfig(saved.id)
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
        McpServerDrawerStore.openMcpServerDrawer(
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
        ? await SystemMcpServer.testSystemServerConnection(payload)
        : await McpServerStore.testMcpServerConnection(payload)
      if (result.success) {
        message.success(result.message || 'Connection successful')
      } else {
        message.error(result.message || 'Connection failed')
      }

      // The probe (sent with `id`) is recorded onto the persisted row's
      // `last_health_check_*` by the backend. Re-fetch + re-bind the drawer to
      // edit mode so `editingServer` reflects the fresh status — this is what
      // makes the top-of-body health-error Alert appear after a failed probe
      // (it renders only on `last_health_check_status === 'unhealthy'`).
      try {
        const fresh = saved.is_system
          ? await SystemMcpServer.getSystemServerById(saved.id)
          : await McpServerStore.getMcpServer(saved.id)
        if (fresh) {
          McpServerDrawerStore.openMcpServerDrawer(
            fresh,
            fresh.is_system ? 'edit-system' : 'edit',
          )
        }
      } catch (e) {
        console.warn('Failed to refresh MCP server after probe:', e)
      }
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Connection test failed',
      )
    } finally {
      setTesting(false)
    }
  }

  const handleSubmit = async () => {
    try {
      McpServerDrawerStore.setMcpServerDrawerLoading(true)
      const saved = await persistServer()
      if (!saved) return
      McpServerDrawerStore.closeMcpServerDrawer()
      form.reset()
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
      message.error(`Failed to save MCP server: ${reason}`, { duration: 8000 })
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
            const fresh = await McpServerStore.getMcpServer(editingServer.id)
            form.setValue('enabled' as any, fresh.enabled)
            setEnabledValue(!!fresh.enabled)
            McpServerDrawerStore.openMcpServerDrawer(fresh, 'edit')
          } catch (e) {
            console.warn('Failed to refresh server after health check:', e)
          }
        } else if (mode === 'edit-system' && editingServer) {
          try {
            const fresh = await SystemMcpServer.getSystemServerById(
              editingServer.id,
            )
            if (fresh) {
              form.setValue('enabled' as any, fresh.enabled)
              setEnabledValue(!!fresh.enabled)
              McpServerDrawerStore.openMcpServerDrawer(fresh, 'edit-system')
            }
          } catch (e) {
            console.warn('Failed to refresh system server after health check:', e)
          }
        }
      }
    } finally {
      McpServerDrawerStore.setMcpServerDrawerLoading(false)
    }
  }

  const handleClose = () => {
    McpServerDrawerStore.closeMcpServerDrawer()
    form.reset()
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

  // Watch reactive form fields that drive conditional rendering.
  // Replaces Form.useWatch and Form.Item shouldUpdate render-props.
  const transportType = useWatch({ control: form.control, name: 'transport_type' as any })
  const runInSandbox = useWatch({ control: form.control, name: 'run_in_sandbox' as any })
  const oauthEnabled = useWatch({ control: form.control, name: 'oauth_enabled' as any })
  const supportsSampling = useWatch({ control: form.control, name: 'supports_sampling' as any })

  // A stdio server runs sandboxed (any command allowed) when:
  //   - system mode + admin toggled run_in_sandbox on, OR
  //   - user mode + stdio (user policy force-sandboxes user stdio;
  //     the backend overrides `run_in_sandbox=true` regardless of
  //     what the FE sends, so we treat user stdio as sandboxed for
  //     the form's command-tier validator).
  // Otherwise the command must be in the host allowlist.
  const isSandboxed =
    transportType === 'stdio' && (!isSystemMode || runInSandbox === true)

  // Clear the prefill-swap Alert once the user has intentionally
  // moved away from the auto-substituted transport — otherwise the
  // banner shows stale "changed from X to Y" text after the user
  // has already chosen a third option.
  useEffect(() => {
    if (
      prefillTransportSwapped &&
      transportType &&
      transportType !== prefillTransportSwapped.to
    ) {
      setPrefillTransportSwapped(null)
    }
  }, [transportType, prefillTransportSwapped])

  // Toggling run_in_sandbox (system mode) flips whether the host command
  // allowlist applies — sandbox lifts it, so a host-disallowed command becomes
  // valid (and vice-versa). RHF won't re-check `command` on its own when a
  // *different* field changes, so re-validate it here to clear/raise the error.
  useEffect(() => {
    if (
      form.getFieldState('command').isTouched ||
      form.formState.isSubmitted
    ) {
      void form.trigger('command')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isSandboxed])

  // Local mirror for the title's Enabled Switch + a "currently
  // toggling" flag that disables the Switch (with a loading
  // spinner) while a save+probe round-trip is in flight.
  // Watching from OUTSIDE the <Form> provider tree was flaky;
  // local state + sync-from-form on open is the robust shape.
  const [enabledValue, setEnabledValue] = useState(false)
  const [togglingEnable, setTogglingEnable] = useState(false)

  // Sync the local mirror with the form's `enabled` field whenever
  // the drawer opens / switches mode / loads a new server.
  useEffect(() => {
    if (!open) return
    if (mode === 'edit' || mode === 'edit-system') {
      setEnabledValue(!!editingServer?.enabled)
    } else {
      // Create mode — default to enabled=true.
      setEnabledValue(true)
    }
  }, [open, mode, editingServer])

  // Timeout is rendered at the END of each transport-specific block
  // (after env vars for stdio, after headers for http/sse — before
  // OAuth). Same FormField name so form state binds regardless of
  // which branch renders. Only one branch is mounted at a time
  // (gated on transportType), so the duplication is purely JSX
  // shape, not state.
  const timeoutField = (
    <FormField
      name="timeout_seconds"
      label="Timeout (seconds)"
      description="Maximum time to wait for a tool call response. Increase for servers that use sampling (multiple LLM calls)."
    >
      <InputNumber
        min={1}
        max={600}
        placeholder="30"
        className="w-full"
        data-testid="mcp-drawer-timeout-input"
      />
    </FormField>
  )

  // Click handler for the title's Enabled Switch — drives the
  // server's enable lifecycle directly from the title.
  //
  // Behaviors per direction:
  //   ON  — save the full current form state + force enabled=true;
  //         backend probes the persisted state.
  //   OFF — minimal PUT with just `enabled: false`.
  //
  // Create mode: runs the connection-test endpoint against the form
  // values WITHOUT persisting a row.
  const handleEnabledToggle = async (v: boolean) => {
    if (mode === 'create' || mode === 'create-system') {
      if (v === false) {
        setEnabledValue(false)
        form.setValue('enabled' as any, false)
        return
      }

      // Basic pre-flight check so the user sees a meaningful prompt
      // instead of a probe error about an empty command or URL.
      const vals = form.getValues()
      const missingRequired =
        !vals.display_name?.trim() ||
        !vals.transport_type ||
        (vals.transport_type === 'stdio' && !vals.command?.trim()) ||
        ((vals.transport_type === 'http' || vals.transport_type === 'sse') &&
          !vals.url?.trim())
      if (missingRequired) {
        setEnabledValue(false)
        form.setValue('enabled' as any, false)
        return
      }

      setTogglingEnable(true)
      try {
        // Build a no-id TestMcpConnectionRequest from form values.
        // No `id` field → backend treats it as a one-shot ephemeral probe.
        const oauth = vals.oauth_enabled
          ? {
              client_id: (vals.oauth_client_id ?? '').trim(),
              client_secret: vals.oauth_client_secret ?? '',
              scopes: (vals.oauth_scopes ?? '').trim() || undefined,
            }
          : undefined
        const payload: TestMcpConnectionRequest = {
          transport_type: vals.transport_type as TransportType,
          command: vals.command || undefined,
          args: Array.isArray(vals.args) ? vals.args : [],
          environment_variables_entries:
            vals.environment_variables_entries ?? [],
          url: vals.url || undefined,
          headers_entries: vals.headers_entries ?? [],
          timeout_seconds: vals.timeout_seconds ?? 30,
          oauth,
        }
        const result =
          mode === 'create-system'
            ? await SystemMcpServer.testSystemServerConnection(payload)
            : await McpServerStore.testMcpServerConnection(payload)
        if (result.success) {
          setEnabledValue(true)
          form.setValue('enabled' as any, true)
          message.success(
            result.message || 'Connection test passed — enabled in form',
          )
        } else {
          setEnabledValue(false)
          form.setValue('enabled' as any, false)
          message.error(result.message ||
              'Connection test failed; server will be created disabled', { duration: 8000 })
        }
      } catch (error) {
        setEnabledValue(false)
        form.setValue('enabled' as any, false)
        const reason =
          error instanceof Error && error.message
            ? error.message
            : 'Connection test failed'
        message.error(reason, { duration: 8000 })
      } finally {
        setTogglingEnable(false)
      }
      return
    }
    if (!editingServer) return

    setTogglingEnable(true)
    try {
      if (v === false) {
        // Minimal PUT — only the enabled flag, leave everything
        // else alone.
        const payload: UpdateMcpServerRequest = { enabled: false }
        const updated =
          mode === 'edit'
            ? await McpServerStore.updateMcpServer(editingServer.id, payload)
            : await SystemMcpServer.updateSystemServer(
                editingServer.id,
                payload,
              )
        setEnabledValue(false)
        form.setValue('enabled' as any, false)
        McpServerDrawerStore.openMcpServerDrawer(updated, mode)
        message.success('Server disabled')
        return
      }

      // ON path — save the full current form (including the new
      // enabled=true) so the backend probes against the user's
      // intended config, not the stale persisted state.
      form.setValue('enabled' as any, true)
      setEnabledValue(true)
      try {
        const saved = await persistServer()
        if (!saved) {
          // Form validation failed — revert the optimistic switch.
          setEnabledValue(false)
          form.setValue('enabled' as any, false)
          return
        }
        McpServerDrawerStore.openMcpServerDrawer(saved, mode)
        message.success('Server enabled — connection test passed')
      } catch (error) {
        // Most likely cause: MCP_ENABLE_FAILED_HEALTH_CHECK from
        // the probe. Surface the reason verbatim, then refresh from
        // the backend so the local mirror reflects the persisted state.
        const reason =
          error instanceof Error && error.message
            ? error.message
            : 'Unknown error'
        message.error(`Failed to enable: ${reason}`, { duration: 8000 })
        try {
          const fresh =
            mode === 'edit'
              ? await McpServerStore.getMcpServer(editingServer.id)
              : SystemMcpServer.getSystemServerById(editingServer.id)
          if (fresh) {
            setEnabledValue(!!fresh.enabled)
            form.setValue('enabled' as any, fresh.enabled)
            McpServerDrawerStore.openMcpServerDrawer(fresh, mode)
          } else {
            setEnabledValue(false)
            form.setValue('enabled' as any, false)
          }
        } catch {
          setEnabledValue(false)
          form.setValue('enabled' as any, false)
        }
      }
    } finally {
      setTogglingEnable(false)
    }
  }

  // Title with the server-Enabled toggle on the right — keeps the
  // on/off control visible no matter how far the user scrolls.
  // The Tooltip also surfaces the persisted last-health-check info.
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
  // The Enabled switch used to live here in the title; it now sits in the form
  // body right after the name (see below), so the header is just the title text.
  const titleNode = getTitle()

  const detailsBody = (
      <div className="flex flex-col gap-4">
        {/* Surface the last probe's failure reason at the top of
            the body as an Alert so it can't be missed. Renders only
            on unhealthy + only in edit mode. */}
        {(mode === 'edit' || mode === 'edit-system') &&
          editingServer?.last_health_check_status === 'unhealthy' && (
            <Alert
              tone="error"
              data-testid="mcp-drawer-health-alert"
              title={
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
          form={form}
          onSubmit={() => {}}
          name="mcp-server-form"
          layout="vertical"
          disabled={!canManage}
          data-testid="mcp-drawer-form"
        >
          {/* Name (only for create mode) */}
          {(mode === 'create' || mode === 'create-system') && (
            <FormField
              label="Name"
              name="name"
              required
            >
              <Input placeholder="e.g., filesystem, fetch, custom-tool" data-testid="mcp-drawer-name-input" />
            </FormField>
          )}

          {/* Display Name */}
          <FormField
            label="Display Name"
            name="display_name"
            required
          >
            <Input placeholder="e.g., Filesystem Access, Web Fetch" data-testid="mcp-drawer-display-name-input" />
          </FormField>

          {/* Enabled — right after the name, not in the header. Controlled
              externally (enabledValue + handleEnabledToggle: edit-mode toggles
              persist + probe), so it's a plain labeled row, not a form-bound
              FormField. */}
          {(!!editingServer || mode === 'create' || mode === 'create-system') && (
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium">Enabled</span>
              <Tooltip
                title={
                  <span style={{ whiteSpace: 'pre-line' }}>
                    {formatHealthTooltip()}
                  </span>
                }
              >
                <Switch
                  aria-label="Enable server"
                  checked={enabledValue}
                  loading={togglingEnable}
                  disabled={!canManage || togglingEnable}
                  onChange={handleEnabledToggle}
                  data-testid="mcp-drawer-enabled-switch"
                />
              </Tooltip>
            </div>
          )}

          {/* Description */}
          <FormField label="Description" name="description">
            <Textarea
              placeholder="Brief description of what this server does"
              rows={2}
              data-testid="mcp-drawer-description-textarea"
            />
          </FormField>

          {/* Surface a Hub-prefill / policy mismatch swap. */}
          {prefillTransportSwapped && (
            <Alert
              tone="info"
              data-testid="mcp-drawer-transport-swap-alert"
              title={`Transport changed from "${prefillTransportSwapped.from}" to "${prefillTransportSwapped.to}"`}
              description="Administrator policy doesn't allow the original transport for user-installed MCP servers. The drawer pre-filled the first permitted transport so you can review and save."
              className="mb-3"
            />
          )}

          {/* Transport Type. In user mode the options are filtered by
              the MCP user policy's allowed_transports. */}
          <FormField
            label="Transport Type"
            name="transport_type"
            required
          >
            <Select
              data-testid="mcp-drawer-transport-select"
              disabled={mode === 'edit' || mode === 'edit-system'}
              options={TRANSPORT_TYPES.filter(type =>
                isUserMode
                  ? policyAllowedTransports.includes(type.value)
                  : true,
              ).map(type => ({
                ...type,
                disabled:
                  (mode === 'edit' || mode === 'edit-system') && editingServer
                    ? editingServer.transport_type !== type.value
                    : false,
              }))}
            />
          </FormField>

          {/* Transport-specific fields */}
          {transportType === 'stdio' && (
            <>
              <FormField
                label="Command"
                name="command"
                required
                description={
                  isSandboxed
                    ? 'Runs in the sandbox — any command is allowed.'
                    : `Allowed on host: ${hostCommands.join(', ')}. Enable "Run in sandbox" to use any command.`
                }
              >
                <Input placeholder="e.g., npx, uvx, node" data-testid="mcp-drawer-command-input" />
              </FormField>

              <FormField
                label="Arguments"
                name="args"
                description='JSON array format, e.g., ["-y", "@modelcontextprotocol/server-filesystem"]'
              >
                <Textarea
                  placeholder='["-y", "@modelcontextprotocol/server-filesystem"]'
                  rows={3}
                  className="font-mono text-xs"
                  data-testid="mcp-drawer-args-textarea"
                />
              </FormField>

              <div className="flex flex-col gap-1.5">
                <p className="text-sm font-medium">Environment Variables</p>
                <KeyValueSecretEditor
                  name="environment_variables_entries"
                  defaultIsSecret={true}
                  keyPlaceholder="GITHUB_TOKEN"
                  valuePlaceholder="value"
                  labelSingular="env var"
                />
                <p className="text-xs text-muted-foreground">
                  One row per variable. Toggle 🔒 to encrypt at rest; secret values are never returned to the client after save (leave blank to keep).
                </p>
              </div>

              {timeoutField}
            </>
          )}

          {(transportType === 'http' || transportType === 'sse') && (
            <>
              <FormField
                label="URL"
                name="url"
                required
              >
                <Input placeholder="https://example.com/mcp" data-testid="mcp-drawer-url-input" />
              </FormField>

              <div className="flex flex-col gap-1.5">
                <p className="text-sm font-medium">HTTP Headers</p>
                <KeyValueSecretEditor
                  name="headers_entries"
                  defaultIsSecret={false}
                  keyPlaceholder="Authorization"
                  valuePlaceholder="Bearer …"
                  labelSingular="header"
                />
                <p className="text-xs text-muted-foreground">
                  One row per header. Toggle 🔒 to encrypt at rest (recommended for tokens / API keys). {"`${VAR}`"} interpolation against env vars is supported in header values.
                </p>
              </div>

              {timeoutField}

              {transportType === 'http' && isUserMode && (
                <>
                  <Separator className="!mt-8">
                    OAuth 2.1
                  </Separator>
                  {/* Section-level enable toggle at the TOP of the
                      OAuth block. When off, the client id / secret /
                      scopes fields disappear entirely — keeps the
                      drawer compact for the common case (no OAuth)
                      and makes "turn off OAuth on this server"
                      explicit (saving with this off clears the
                      stored OAuth config). */}
                  <FormField
                    label="Enable OAuth 2.1"
                    name="oauth_enabled"
                    valuePropName="checked"
                    description="For servers requiring OAuth client_credentials. Turning this off on save clears any stored OAuth config."
                  >
                    <Switch data-testid="mcp-drawer-oauth-enabled-switch" />
                  </FormField>
                  {oauthEnabled ? (
                    <>
                      <FormField
                        label="OAuth Client ID"
                        name="oauth_client_id"
                        description="Client ID issued by the upstream OAuth server."
                      >
                        <Input placeholder="client id" autoComplete="off" data-testid="mcp-drawer-oauth-client-id-input" />
                      </FormField>
                      <FormField
                        label="OAuth Client Secret"
                        name="oauth_client_secret"
                        description={
                          hasExistingOAuth
                            ? 'A secret is stored. Leave blank to keep it; enter a value to replace it.'
                            : 'Stored securely and never shown again.'
                        }
                      >
                        <PasswordInput
                          placeholder={hasExistingOAuth ? '•••••••• (unchanged)' : 'client secret'}
                          autoComplete="new-password"
                          showLabel="Show secret"
                          hideLabel="Hide secret"
                          data-testid="mcp-drawer-oauth-secret-input"
                        />
                      </FormField>
                      <FormField
                        label="OAuth Scopes"
                        name="oauth_scopes"
                        description="Optional, space-separated (e.g. 'mcp read')."
                      >
                        <Input placeholder="mcp" autoComplete="off" data-testid="mcp-drawer-oauth-scopes-input" />
                      </FormField>
                    </>
                  ) : null}
                </>
              )}
            </>
          )}

          {/* `Enabled` lives in the drawer title now (always visible
              regardless of scroll). `Timeout` lives at the end of
              each transport-specific block. Sampling stays here as
              its own section because it's transport-agnostic. */}
          <Separator className="!mt-8">Sampling</Separator>

          {/* Supports Sampling */}
          <FormField
            label="Enable MCP Sampling"
            name="supports_sampling"
            valuePropName="checked"
            description="Allow this server to request LLM completions inline during tool execution (requires HTTP transport and server support)"
          >
            <Switch data-testid="mcp-drawer-sampling-switch" />
          </FormField>

          {/* Sampling sub-fields — only meaningful when sampling is
              enabled; hide entirely otherwise to reduce noise. */}
          {supportsSampling ? (
            <>
              <FormField
                label="Usage Mode"
                name="usage_mode"
                description="Auto: LLM decides when to call this server. Always: server is called before every LLM request to enrich context."
              >
                <Select
                  data-testid="mcp-drawer-usage-mode-select"
                  options={[
                    { label: 'Auto (LLM decides)', value: 'auto' },
                    { label: 'Always (pre-process every prompt)', value: 'always' },
                  ]}
                />
              </FormField>

              <FormField
                label="Max Concurrent Sessions"
                name="max_concurrent_sessions"
                description="Limit simultaneous sampling sessions. Leave blank for unlimited. Users over the limit receive a friendly error."
              >
                <InputNumber min={1} placeholder="Unlimited" className="w-full" data-testid="mcp-drawer-max-sessions-input" />
              </FormField>
            </>
          ) : null}

          {/* Run in sandbox + flavor (system + stdio). Admin toggles
              run_in_sandbox; when on, the flavor Select shows. Toggle
              re-validates the command field because turning sandbox on
              lifts the host command allowlist (any command is OK
              inside bwrap). */}
          {transportType === 'stdio' &&
            (mode === 'create-system' || mode === 'edit-system') && (
              <>
                <FormField
                  label="Run in sandbox"
                  name="run_in_sandbox"
                  valuePropName="checked"
                  description={
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
                  <Switch data-testid="mcp-drawer-run-sandbox-switch" />
                </FormField>

                {runInSandbox && (
                  <FormField
                    label="Sandbox flavor"
                    name="sandbox_flavor"
                    description="Rootfs image the sandboxed server runs in. 'full' ships Node (npx), uv (uvx), python3 and R; 'minimal' is python3-only."
                  >
                    <Select options={flavorOptions} data-testid="mcp-drawer-sandbox-flavor-select" />
                  </FormField>
                )}
              </>
            )}

          {/* User-mode + stdio: surface the policy-imposed sandbox
              decision so the user understands they cannot opt out. */}
          {isUserMode && transportType === 'stdio' && (
            <Alert
              tone="info"
              data-testid="mcp-drawer-sandbox-info-alert"
              title="Stdio MCP servers run inside the sandbox"
              description={
                <>
                  Per administrator policy, stdio MCP servers you add
                  are launched inside the{' '}
                  <strong>
                    {policyUserStdioSandboxFlavor ?? 'minimal'}
                  </strong>{' '}
                  code_sandbox flavor. The server only sees an isolated
                  workspace — filesystem-oriented MCP servers will not
                  see your real files.
                </>
              }
              className="mb-3"
            />
          )}
        </Form>
      </div>
  )

  // In edit mode, surface a per-server tool-call history tab beside the form.
  // Create mode has no server id yet, so just render the form.
  const isEditMode = mode === 'edit' || mode === 'edit-system'
  return (
    <Drawer
      open={open}
      onClose={handleClose}
      title={titleNode}
      size={600}
      footer={
        <div className="flex items-center justify-between gap-2">
          <div>
            {canManage && !!transportType && (
              <Button
                variant="outline"
                loading={testing}
                disabled={loading}
                onClick={handleSaveAndTest}
                data-testid="mcp-drawer-save-test-btn"
              >
                Save &amp; Test Connection
              </Button>
            )}
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={handleClose} data-testid="mcp-drawer-cancel-btn">
              {canManage ? 'Cancel' : 'Close'}
            </Button>
            {canManage && (
              <Button
                loading={loading}
                disabled={testing}
                onClick={handleSubmit}
                data-testid="mcp-drawer-submit-btn"
              >
                {getButtonText()}
              </Button>
            )}
          </div>
        </div>
      }
    >
      {isEditMode && editingServer ? (
        <Tabs
          defaultValue="details"
          data-testid="mcp-drawer-tabs"
          items={[
            { key: 'details', label: 'Details', children: detailsBody },
            {
              key: 'calls',
              label: 'Calls',
              children: <McpToolCallsTab serverId={editingServer.id} />,
            },
          ]}
        />
      ) : (
        detailsBody
      )}
    </Drawer>
  )
}
