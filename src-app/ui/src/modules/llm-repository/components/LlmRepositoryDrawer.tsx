import {
  CloudDownloadOutlined,
  EyeInvisibleOutlined,
  EyeTwoTone,
} from '@ant-design/icons'
import {
  Alert,
  App,
  Button,
  Form,
  Input,
  Select,
  Switch,
  Typography,
} from 'antd'
import { useEffect, useState } from 'react'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type CreateLlmRepositoryRequest,
  type UpdateLlmRepositoryRequest,
} from '@/api-client/types'

const { Text } = Typography

export function LlmRepositoryDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)
  // Local mirror of the Switch's checked state. Form.useWatch is
  // flaky for elements rendered outside the immediate Form provider
  // tree, and we also want a non-form-state value the
  // save-then-probe-then-revert path can read+write without
  // triggering a form re-render storm. Mirrors the MCP drawer pattern
  // at `McpServerDrawer.tsx:604`.
  const [enabledValue, setEnabledValue] = useState(false)
  // Loading state for the in-place enable transition (edit mode).
  const [togglingEnable, setTogglingEnable] = useState(false)

  const { creating, updating, testing } = Stores.LlmRepository
  const { open, editingRepository: repository } = Stores.LlmRepositoryDrawer
  const canCreate = usePermission(Permissions.LlmRepositoriesCreate)
  const canEdit = usePermission(Permissions.LlmRepositoriesEdit)
  // Effective gate on the form: editing requires edit; creating requires create.
  const canSave = repository ? canEdit : canCreate
  const mode: 'create' | 'edit' = repository ? 'edit' : 'create'

  // Update form when editing repository.
  //
  // NOTE: api_key / password / token are no longer returned in GET
  // responses (09-llm-repository F-02 closure — credentials were
  // exposed to every user with read access). They're write-only:
  // empty in the form means "keep existing"; the user enters a new
  // value to replace. Username + auth_test_api_endpoint remain
  // visible (non-secret).
  useEffect(() => {
    if (repository && open) {
      form.setFieldsValue({
        name: repository.name,
        url: repository.url,
        auth_type: repository.auth_type,
        username: repository.auth_config?.username,
        auth_test_api_endpoint: repository.auth_config?.auth_test_api_endpoint,
        enabled: repository.enabled,
      })
      setEnabledValue(repository.enabled)
    } else if (!repository && open) {
      form.setFieldsValue({
        auth_type: 'none',
        enabled: true,
      })
      setEnabledValue(true)
    }
  }, [repository, open, form])

  /**
   * Pre-flight validation for the form-only test path (CREATE mode +
   * create-mode Enable switch). Edit mode skips the secret-required
   * checks because the persisted (decrypted) secret will fill in on
   * the server side; the user only re-types secrets when they want
   * to rotate.
   *
   * Returns `{ ok: true }` when the form is complete enough to probe,
   * or `{ ok: false, hint }` with a human-readable warning the caller
   * can surface in a toast.
   */
  const validateFormForTest = (
    values: any,
    skipSecretChecks: boolean,
  ): { ok: true } | { ok: false; hint: string } => {
    if (!values.name) {
      return { ok: false, hint: 'Please enter a repository name first' }
    }
    if (!values.url) {
      return { ok: false, hint: 'Please enter a repository URL first' }
    }
    if (skipSecretChecks) {
      return { ok: true }
    }
    if (values.auth_type === 'api_key' && !values.api_key) {
      return { ok: false, hint: 'Please enter an API key first' }
    }
    if (
      values.auth_type === 'basic_auth' &&
      (!values.username || !values.password)
    ) {
      return { ok: false, hint: 'Please enter username and password first' }
    }
    if (values.auth_type === 'bearer_token' && !values.token) {
      return { ok: false, hint: 'Please enter a bearer token first' }
    }
    return { ok: true }
  }

  /**
   * "Test Connection" button. Mode-aware:
   *
   * - **CREATE mode**: posts the form values to the form-only test
   *   endpoint, which probes without persisting anything. Toast only.
   *
   * - **EDIT mode**: posts the form values (as `overrides`) to the
   *   by-id endpoint. The backend merges the overrides over the
   *   persisted row, falling back to the saved decrypted secret for
   *   write-only fields the user didn't re-type. The outcome is
   *   recorded to `last_health_check_*` columns server-side; on a
   *   currently-enabled row that fails, the row is auto-disabled.
   *   The store's `updated` / `auto_disabled` listeners re-sync the
   *   drawer's editing row + the list page.
   */
  const testRepositoryFromForm = async () => {
    const values = form.getFieldsValue()
    const isEdit = mode === 'edit'

    // EDIT mode skips secret-required checks because the backend
    // falls back to the persisted decrypted secret; the user
    // doesn't have to re-type the api_key to test the saved row.
    const validation = validateFormForTest(values, isEdit)
    if (!validation.ok) {
      message.warning(validation.hint)
      return
    }

    try {
      const overrides = {
        name: values.name,
        url: values.url,
        auth_type: values.auth_type,
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
      }

      const result = isEdit && repository
        ? await Stores.LlmRepository.testLlmRepositoryById(
            repository.id,
            overrides,
          )
        : await Stores.LlmRepository.testLlmRepositoryConnection(overrides)

      if (result.success) {
        message.success(
          result.message || `Connection to ${values.name} successful!`,
        )
      } else {
        // 8s for failure so the user has time to read the reason —
        // matches the longer-duration pattern used for failed enable
        // transitions elsewhere in this drawer.
        message.error({
          content:
            result.message || `Connection to ${values.name} failed`,
          duration: 8,
        })
      }
    } catch (error: any) {
      console.error('Repository connection test failed:', error)
      message.error({
        content: error?.message || `Connection to ${values.name} failed`,
        duration: 8,
      })
    }
  }

  const handleClose = () => {
    form.resetFields()
    Stores.LlmRepositoryDrawer.closeDrawer()
  }

  /**
   * Persist the form's current state. Used by both the bottom Save/Add
   * button (`handleSubmit`) and the title Enabled Switch's "save full
   * form + probe" ON path (`handleEnabledToggle`). The `enabledOverride`
   * lets the toggle path force `enabled = true` even when the form
   * value is stale relative to local state.
   *
   * Returns the saved repository on success; throws on failure (the
   * backend's `enforce_on_update_transition` returns 400 with the
   * probe reason, which the catch surfaces in a toast).
   */
  const persistRepository = async (
    values: any,
    enabledOverride?: boolean,
  ) => {
    let repositoryData: UpdateLlmRepositoryRequest

    if (repository?.built_in) {
      // Built-in: only auth fields are mutable.
      repositoryData = {
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
      }
      if (enabledOverride !== undefined) {
        repositoryData.enabled = enabledOverride
      }
    } else {
      repositoryData = {
        name: values.name,
        url: values.url,
        auth_type: values.auth_type,
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
        enabled: enabledOverride ?? (values.enabled ?? true),
      }
    }

    if (repository) {
      return Stores.LlmRepository.updateLlmRepository(
        repository.id,
        repositoryData,
      )
    } else {
      const createData: CreateLlmRepositoryRequest = {
        name: values.name,
        url: values.url,
        auth_type: values.auth_type,
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
        enabled: enabledOverride ?? (values.enabled ?? true),
      }
      const wrapped = await Stores.LlmRepository.createLlmRepository(createData)
      // Surface the create-flow probe outcome to the user. The wrapper
      // is flattened: LlmRepository fields are at top level (so
      // `wrapped` IS the canonical row with the auto-downgraded
      // `enabled` value), and `connection_warning` is an optional
      // sibling that appears only when the probe failed.
      if (wrapped.connection_warning) {
        message.warning({
          content: `Repository added but disabled — ${wrapped.connection_warning.reason}`,
          duration: 8,
        })
      }
      // Strip `connection_warning` so the caller sees a plain
      // LlmRepository shape — the warning has already been surfaced.
      const { connection_warning: _w, ...repository } = wrapped
      return repository
    }
  }

  const handleSubmit = async (values: any) => {
    setLoading(true)
    try {
      await persistRepository(values)
      message.success(
        repository ? 'Repository updated successfully' : 'Repository added successfully',
      )
      handleClose()
    } catch (error: any) {
      console.error('Failed to save repository:', error)
      message.error({
        content: error?.message || 'Failed to save repository',
        duration: 8,
      })
    } finally {
      setLoading(false)
    }
  }

  /**
   * Drawer Enabled-Switch behavior:
   *
   * - **Create mode, ON**: runs the form-only test endpoint
   *   immediately — no row is created. If the probe passes, the
   *   Switch sticks ON (the bottom Add button will persist later);
   *   if it fails, the Switch snaps back OFF and a toast surfaces
   *   the reason. Pre-flight validates that the URL + auth fields
   *   are filled in — empty form → warning toast, switch stays OFF.
   *
   * - **Create mode, OFF**: just local state.
   *
   * - **Edit mode, OFF**: minimal PUT `{ enabled: false }`. No probe.
   *   Other in-flight form edits stay in the form, picked up by the
   *   next explicit Save click.
   *
   * - **Edit mode, ON**: full-form save via `persistRepository(..., true)`.
   *   Backend probes via `enforce_on_update_transition`; on 400, the
   *   `llm_repository.auto_disabled` event flows back through the store
   *   and the drawer's `editingRepository` re-syncs from the canonical
   *   row. We re-mirror local state from that fresh row + show the
   *   reason in a longer-lived toast.
   */
  const handleEnabledToggle = async (v: boolean) => {
    if (mode === 'create') {
      if (v === false) {
        // OFF in create mode is purely local — there's nothing
        // persisted to disable.
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        return
      }

      // ON in create mode: probe the form values without persisting.
      // Mirrors the user's request that the Switch in the Add Repository
      // drawer "immediately test the connection without saving".
      const values = form.getFieldsValue()
      const validation = validateFormForTest(values, false)
      if (!validation.ok) {
        // Stay OFF; show the hint so the user knows what's missing.
        message.warning(validation.hint)
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        return
      }

      setTogglingEnable(true)
      try {
        const result = await Stores.LlmRepository.testLlmRepositoryConnection({
          name: values.name,
          url: values.url,
          auth_type: values.auth_type,
          auth_config: {
            api_key: values.api_key,
            username: values.username,
            password: values.password,
            token: values.token,
            auth_test_api_endpoint: values.auth_test_api_endpoint,
          },
        })
        if (result.success) {
          setEnabledValue(true)
          form.setFieldsValue({ enabled: true })
          message.success(
            result.message || 'Connection test passed — enabled in form',
          )
        } else {
          setEnabledValue(false)
          form.setFieldsValue({ enabled: false })
          message.error({
            content:
              result.message ||
              'Connection test failed; repository will be created disabled',
            duration: 8,
          })
        }
      } catch (error: any) {
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        message.error({
          content: error?.message || 'Connection test failed',
          duration: 8,
        })
      } finally {
        setTogglingEnable(false)
      }
      return
    }

    if (!repository) return // type-narrow; edit mode always has a row

    setTogglingEnable(true)
    try {
      if (v === false) {
        // OFF path — minimal PUT; no probe runs server-side.
        await Stores.LlmRepository.updateLlmRepository(repository.id, {
          enabled: false,
        })
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        message.success('Repository disabled')
        return
      }

      // ON path — save the full form (forcing enabled=true). Backend
      // probes the persisted config; on failure, the response is 400
      // and the AutoDisabled event flips the row back to disabled
      // via the store's event listener.
      const values = form.getFieldsValue()
      form.setFieldsValue({ enabled: true })
      setEnabledValue(true)
      try {
        await persistRepository(values, true)
        message.success('Repository enabled — connection test passed')
      } catch (error: any) {
        // The store + drawer store listen for `llm_repository.updated`
        // and `llm_repository.auto_disabled` respectively, so the row's
        // canonical state (enabled=false, status='unhealthy', etc.)
        // is already in the drawer's `editingRepository` by the time
        // we get here. Mirror it back to local state so the Switch
        // snaps off + the Alert renders.
        const reason =
          error?.message ||
          'Connection probe failed; repository remains disabled.'
        setEnabledValue(false)
        form.setFieldsValue({ enabled: false })
        message.error({
          content: `Failed to enable: ${reason}`,
          duration: 8,
        })
      }
    } finally {
      setTogglingEnable(false)
    }
  }

  return (
    <Drawer
      title={
        repository
          ? repository.built_in
            ? 'Edit Built-in Repository (Authentication Only)'
            : 'Edit Repository'
          : 'Add Repository'
      }
      open={open}
      onClose={handleClose}
      footer={null}
      size={600}
      mask={{ closable: false }}
    >
      {/* Unhealthy Alert at the top of the body so the operator
          immediately sees why a previously-enabled repository is now
          disabled. Renders only on `unhealthy` in edit mode (create
          mode has no probe history to surface). Mirrors the
          McpServerDrawer pattern. */}
      {mode === 'edit' &&
        repository?.last_health_check_status === 'unhealthy' && (
          <Alert
            type="error"
            showIcon
            className="!mb-4"
            message={
              repository.last_health_check_at
                ? `Connection test failed at ${new Date(
                    repository.last_health_check_at,
                  ).toLocaleString()}`
                : 'Connection test failed'
            }
            description={
              repository.last_health_check_reason ?? 'No reason recorded.'
            }
          />
        )}
      <Form
        name="llm-repository-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
        disabled={!canSave}
      >
        <Form.Item
          name="name"
          label="Repository Name"
          rules={[
            { required: true, message: 'Please enter a repository name' },
          ]}
        >
          <Input
            placeholder="My Custom Repository"
            disabled={repository?.built_in}
          />
        </Form.Item>

        <Form.Item
          name="url"
          label="Repository URL"
          rules={[
            { required: true, message: 'Please enter a repository URL' },
            { type: 'url', message: 'Please enter a valid URL' },
          ]}
        >
          <Input
            placeholder="https://your-custom-repo.com/models"
            disabled={repository?.built_in}
          />
        </Form.Item>

        <Form.Item
          name="auth_type"
          label="Authentication Type"
          rules={[{ required: true }]}
        >
          <Select disabled={repository?.built_in}>
            <Select.Option value="none">No Authentication</Select.Option>
            <Select.Option value="api_key">API Key</Select.Option>
            <Select.Option value="basic_auth">
              Basic Authentication
            </Select.Option>
            <Select.Option value="bearer_token">Bearer Token</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item dependencies={['auth_type']} noStyle>
          {({ getFieldValue }) => {
            const authType = getFieldValue('auth_type')

            if (authType === 'api_key') {
              return (
                <Form.Item name="api_key" label="API Key">
                  <Input.Password
                    placeholder="Enter your API key"
                    iconRender={visible =>
                      visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                    }
                  />
                </Form.Item>
              )
            }

            if (authType === 'basic_auth') {
              return (
                <>
                  <Form.Item name="username" label="Username">
                    <Input placeholder="Enter your username" />
                  </Form.Item>
                  <Form.Item name="password" label="Password">
                    <Input.Password
                      placeholder="Enter your password"
                      iconRender={visible =>
                        visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                      }
                    />
                  </Form.Item>
                </>
              )
            }

            if (authType === 'bearer_token') {
              return (
                <Form.Item name="token" label="Bearer Token">
                  <Input.Password
                    placeholder="Enter your bearer token"
                    iconRender={visible =>
                      visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                    }
                  />
                </Form.Item>
              )
            }

            return null
          }}
        </Form.Item>

        <Form.Item
          name="auth_test_api_endpoint"
          label="Authentication Test Endpoint"
          extra="Custom endpoint to test authentication. If not provided, the main repository URL will be used for testing."
        >
          <Input
            disabled={repository?.built_in}
            placeholder="https://api.example.com/auth/test"
          />
        </Form.Item>

        {/* Test Connection Section */}
        <Form.Item
          dependencies={[
            'url',
            'auth_type',
            'api_key',
            'username',
            'password',
            'token',
            'auth_test_api_endpoint',
          ]}
          noStyle
        >
          {({ getFieldValue }) => {
            const authType = getFieldValue('auth_type')
            const url = getFieldValue('url')

            // Only show test button if URL is provided and auth is configured (if needed)
            const showTestButton =
              url &&
              (authType === 'none' ||
                (authType === 'api_key' && getFieldValue('api_key')) ||
                (authType === 'basic_auth' &&
                  getFieldValue('username') &&
                  getFieldValue('password')) ||
                (authType === 'bearer_token' && getFieldValue('token')))

            if (showTestButton) {
              return (
                <Form.Item label="Connection Test">
                  <div>
                    <Text type="secondary" className="block mb-3">
                      Test your repository configuration to ensure it's
                      accessible
                    </Text>
                    <Button
                      type="default"
                      icon={<CloudDownloadOutlined />}
                      loading={testing}
                      onClick={testRepositoryFromForm}
                    >
                      Test Connection
                    </Button>
                  </div>
                </Form.Item>
              )
            }

            return null
          }}
        </Form.Item>

        {/* Enable Repository switch.
         *
         * Hidden form field carries the value into the form state so
         * `persistRepository` still sees a populated `values.enabled`.
         * The visible Switch is driven by local `enabledValue` so the
         * save-then-probe-then-revert flow can snap it back without
         * waiting for a form re-render.
         *
         * In CREATE mode: Switch toggles local state + form value; the
         * bottom Add button persists.
         * In EDIT mode: Switch immediately PUTs the change (OFF = minimal
         * disable, ON = save full form + backend probe with auto-revert
         * on probe failure).
         */}
        <Form.Item name="enabled" hidden valuePropName="checked">
          <Switch />
        </Form.Item>
        <Form.Item label="Enable Repository">
          <Switch
            checked={enabledValue}
            disabled={repository?.built_in}
            loading={togglingEnable}
            onChange={handleEnabledToggle}
            aria-label="Enable repository"
          />
          {mode === 'edit' && (
            <Text type="secondary" className="block mt-1 text-xs">
              Enabling runs a connection probe; the repository stays
              disabled if it can't reach the upstream.
            </Text>
          )}
        </Form.Item>

        <div className="flex justify-end gap-3 pt-4">
          <Button
            onClick={handleClose}
            disabled={loading || creating || updating}
          >
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button
              type="primary"
              htmlType="submit"
              loading={loading || creating || updating}
            >
              {repository ? 'Save' : 'Add'}
            </Button>
          )}
        </div>
      </Form>
    </Drawer>
  )
}
