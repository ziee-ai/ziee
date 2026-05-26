import { useEffect, useMemo, useState } from 'react'
import {
  Alert,
  App,
  Button,
  Flex,
  Form,
  Input,
  Select,
  Spin,
  Switch,
  Typography,
} from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Permissions } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { usePermission } from '@/core/permissions/usePermission'
import type { AuthProviderResponse, TestProviderResponse } from '@/api-client/types'
import type { ProviderTemplate } from '../types'

const { Title, Paragraph } = Typography

/**
 * Drawer mode: either create-from-template (no `existing` row, has
 * `template`) or edit-existing (has `existing` row, no `template`).
 */
export interface EditDrawerProps {
  open: boolean
  template?: ProviderTemplate
  existing?: AuthProviderResponse
  onClose: () => void
}

interface FormShape {
  name: string
  enabled: boolean
  config: Record<string, any>
}

const SECRET_FIELDS = ['client_secret'] as const

/**
 * Per-provider-type field renderer. OIDC + OAuth2 are similar
 * (client_id/secret + endpoint URLs); Apple is entirely different
 * (team_id/services_id/key_id/.p8 path).
 *
 * Fields are rendered with `disabled={!canManage}` so a user with
 * read-only permission sees the config but can't change it. The
 * submit button is wrapped in `<Can>` so they don't see it at all.
 */
export function AuthProviderEditDrawer({
  open,
  template,
  existing,
  onClose,
}: EditDrawerProps) {
  const { message } = App.useApp()
  const [form] = Form.useForm<FormShape>()
  const { saving, error } = Stores.AuthProvidersAdmin
  const canManage = usePermission(Permissions.AuthProvidersManage)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestProviderResponse | null>(
    null,
  )

  // Resolve the provider type for this drawer instance.
  const providerType = useMemo<string>(
    () => template?.provider_type ?? existing?.provider_type ?? 'oidc',
    [template, existing],
  )

  useEffect(() => {
    if (!open) return
    setTestResult(null)
    if (template) {
      // Auto-fill `name` from the template key for Google / Microsoft
      // / Apple. The migration 47 pre-seed already created rows with
      // those names, so the Add menu disables those entries (see
      // AddProviderMenu.existingNames); this branch only fires for
      // generic templates where `key` doesn't collide.
      const defaultName = templateDefaultName(template)
      form.setFieldsValue({
        name: defaultName,
        // Safer default: rows are created disabled. Admin verifies
        // with Test config + toggles on when ready.
        enabled: false,
        config: template.defaultConfig,
      })
    } else if (existing) {
      form.setFieldsValue({
        name: existing.name,
        enabled: existing.enabled,
        config: existing.config ?? {},
      })
    }
  }, [open, template, existing, form])

  const onTestConfig = async () => {
    setTesting(true)
    setTestResult(null)
    try {
      const values = await form.validateFields()
      const normalized = normalizeConfig(values.config, providerType)
      const res = await Stores.AuthProvidersAdmin.testConfig({
        name: values.name.trim(),
        provider_type: providerType,
        enabled: false,
        config: normalized,
      })
      setTestResult(res)
    } catch {
      // form validation failed; AntD surfaces inline
    } finally {
      setTesting(false)
    }
  }

  const onSubmit = async () => {
    try {
      const values = await form.validateFields()
      // Normalize: scopes can be entered as comma-separated string.
      const normalized = normalizeConfig(values.config, providerType)
      if (template) {
        await Stores.AuthProvidersAdmin.createProvider({
          name: values.name.trim(),
          provider_type: providerType,
          enabled: values.enabled,
          config: normalized,
        })
        message.success(`Created ${values.name.trim()}`)
      } else if (existing) {
        await Stores.AuthProvidersAdmin.updateProvider(existing.id, {
          name: values.name.trim(),
          enabled: values.enabled,
          config: normalized,
        })
        message.success(`Saved ${existing.name}`)
      }
      onClose()
    } catch {
      // store sets `error`; form's own validateFields surfaces inline.
    }
  }

  const titleText = existing
    ? `Edit ${existing.name}`
    : template
      ? `Add ${template.label}`
      : 'New provider'

  return (
    <Drawer
      title={titleText}
      open={open}
      onClose={onClose}
      size={600}
      maskClosable={false}
      destroyOnClose
      footer={
        // Cancel/Close → Save, right-aligned. Matches the dominant
        // convention from 08-cross-cutting-consistency I-1 (every
        // non-user-module drawer uses justify-end + Cancel-then-Submit).
        // Read-only users get a "Close" button instead of "Cancel"
        // because there's nothing to cancel.
        <Flex className="justify-end gap-2">
          <Button onClick={onClose} disabled={saving}>
            {canManage ? 'Cancel' : 'Close'}
          </Button>
          <Can permission={Permissions.AuthProvidersManage}>
            <Button type="primary" loading={saving} onClick={onSubmit}>
              {existing ? 'Save' : 'Create'}
            </Button>
          </Can>
        </Flex>
      }
    >
      {/* The project's <Drawer> wrapper applies `flex w-full` to its
          children container, so multiple top-level children get laid
          out as a flex ROW (alert sits left of the form, breaking
          the layout — see visual smoke test). Wrap in a single column
          container so everything stacks vertically inside the
          drawer body. */}
      <div className="flex flex-col w-full">
        {error && (
          <Alert type="error" message={error} showIcon className="mb-4" />
        )}
        {testing && (
          <div className="text-center py-3 mb-4">
            <Spin /> <Typography.Text type="secondary">Testing…</Typography.Text>
          </div>
        )}
        {testResult && !testing && (
          <Alert
            type={testResult.ok ? 'success' : 'warning'}
            message={testResult.ok ? 'Configuration OK' : 'Configuration issues'}
            description={testResult.message}
            showIcon
            closable
            onClose={() => setTestResult(null)}
            className="mb-4"
          />
        )}
        <Form form={form} layout="vertical" disabled={!canManage} onFinish={onSubmit}>
        <Form.Item
          name="name"
          label="Name (URL slug)"
          rules={[
            { required: true, message: 'Provider name required' },
            {
              // Lowercase only — names are URL slugs and used in
              // log lines. Case-insensitive matching would mean
              // `Google` and `google` look like the same provider
              // on the wire but distinct rows in the DB.
              pattern: /^[a-z0-9-]+$/,
              message:
                'Lowercase letters, digits, and hyphens only (appears in URLs)',
            },
          ]}
        >
          <Input
            placeholder="e.g. google, microsoft-corp, apple"
            disabled={!!existing}
          />
        </Form.Item>
        <Form.Item
          name="enabled"
          label="Enabled"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        <Title level={5} className="mt-2">
          Configuration
        </Title>
        {providerType === 'apple' ? (
          <AppleFields />
        ) : providerType === 'oauth2' ? (
          <OAuth2Fields />
        ) : (
          <OidcFields />
        )}

        {existing && (
          <Paragraph type="secondary" className="mt-3 text-xs">
            Leave <code>client_secret</code> empty to keep the existing value.
          </Paragraph>
        )}
        <Can permission={Permissions.AuthProvidersManage}>
          <Flex className="mt-4">
            <Button loading={testing} onClick={onTestConfig}>
              Test config
            </Button>
          </Flex>
        </Can>
        </Form>
      </div>
    </Drawer>
  )
}

function OidcFields() {
  return (
    <>
      <Form.Item
        name={['config', 'client_id']}
        label="Client ID"
        rules={[{ required: true }]}
      >
        <Input />
      </Form.Item>
      <Form.Item name={['config', 'client_secret']} label="Client secret">
        <Input.Password
          placeholder="••••••  (leave empty to keep existing)"
          autoComplete="new-password"
        />
      </Form.Item>
      <Form.Item
        name={['config', 'issuer_url']}
        label="Issuer URL"
        rules={[{ required: true, type: 'url' }]}
      >
        <Input placeholder="https://accounts.google.com" />
      </Form.Item>
      <Form.Item
        name={['config', 'scopes']}
        label="Scopes"
        getValueFromEvent={parseScopeInput}
        normalize={normalizeScopeArray}
      >
        <Input placeholder="openid email profile" />
      </Form.Item>
      <Form.Item
        name={['config', 'allowed_tenant_ids']}
        label="Allowed tenant IDs (Microsoft only)"
        tooltip="Comma-separated list of tenant IDs (the `tid` claim). REQUIRED when issuer is the Microsoft `common` endpoint."
        getValueFromEvent={parseScopeInput}
        normalize={normalizeScopeArray}
      >
        <Input placeholder="comma-separated UUIDs (leave blank for non-MS)" />
      </Form.Item>
      <Form.Item name={['config', 'display_name']} label="Button label">
        <Input placeholder="Sign in with X" />
      </Form.Item>
    </>
  )
}

function OAuth2Fields() {
  return (
    <>
      <Form.Item
        name={['config', 'client_id']}
        label="Client ID"
        rules={[{ required: true }]}
      >
        <Input />
      </Form.Item>
      <Form.Item name={['config', 'client_secret']} label="Client secret">
        <Input.Password
          placeholder="••••••  (leave empty to keep existing)"
          autoComplete="new-password"
        />
      </Form.Item>
      <Form.Item
        name={['config', 'authorization_url']}
        label="Authorization URL"
        rules={[{ required: true, type: 'url' }]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        name={['config', 'token_url']}
        label="Token URL"
        rules={[{ required: true, type: 'url' }]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        name={['config', 'userinfo_url']}
        label="UserInfo URL"
        rules={[{ type: 'url' }]}
      >
        <Input />
      </Form.Item>
      <Form.Item
        name={['config', 'scopes']}
        label="Scopes"
        getValueFromEvent={parseScopeInput}
        normalize={normalizeScopeArray}
      >
        <Input placeholder="email profile" />
      </Form.Item>
      <Form.Item name={['config', 'display_name']} label="Button label">
        <Input placeholder="Sign in with X" />
      </Form.Item>
    </>
  )
}

function AppleFields() {
  return (
    <>
      <Form.Item
        name={['config', 'team_id']}
        label="Team ID"
        rules={[{ required: true }]}
      >
        <Input placeholder="10-char Apple Developer Team ID" />
      </Form.Item>
      <Form.Item
        name={['config', 'services_id']}
        label="Services ID"
        rules={[{ required: true }]}
      >
        <Input placeholder="The Apple-issued client identifier (Services ID)" />
      </Form.Item>
      <Form.Item
        name={['config', 'key_id']}
        label="Key ID"
        rules={[{ required: true }]}
      >
        <Input placeholder="10-char Key ID for the .p8 file" />
      </Form.Item>
      <Form.Item
        name={['config', 'private_key_path']}
        label="Private key path on disk"
        rules={[{ required: true }]}
        tooltip="Filesystem path to the AuthKey_<KEY_ID>.p8 file. The file itself stays on disk with proper permissions — it is not uploaded through the UI."
      >
        <Input placeholder="/var/lib/ziee/apple/AuthKey_XXXXXXXXXX.p8" />
      </Form.Item>
      <Form.Item
        name={['config', 'scopes']}
        label="Scopes"
        getValueFromEvent={parseScopeInput}
        normalize={normalizeScopeArray}
      >
        <Select
          mode="tags"
          tokenSeparators={[' ', ',']}
          placeholder="name email"
        />
      </Form.Item>
    </>
  )
}

function parseScopeInput(e: any): string[] | string {
  // AntD Select returns array; Input returns event.target.value.
  if (Array.isArray(e)) return e
  if (typeof e === 'string') return e
  return e?.target?.value ?? ''
}

function normalizeScopeArray(value: any): string[] {
  if (Array.isArray(value)) {
    return value
      .map(v => (typeof v === 'string' ? v.trim() : ''))
      .filter(Boolean)
  }
  if (typeof value === 'string') {
    return value
      .split(/[\s,]+/)
      .map(s => s.trim())
      .filter(Boolean)
  }
  return []
}

/// Auto-fill the `name` field for Google / Microsoft / Apple
/// templates so the row matches the migration-47 pre-seed naming.
/// Other templates (generic OIDC/OAuth2) leave it blank for the
/// admin to fill.
function templateDefaultName(t: ProviderTemplate): string {
  if (t.key === 'google' || t.key === 'microsoft' || t.key === 'apple') {
    return t.key
  }
  return ''
}

function normalizeConfig(
  config: Record<string, any>,
  _providerType: string,
): Record<string, any> {
  const out = { ...config }
  // Drop empty secret-ish fields so the backend "leave unchanged"
  // behavior kicks in cleanly.
  for (const k of SECRET_FIELDS) {
    if (typeof out[k] === 'string' && out[k].trim() === '') {
      delete out[k]
    }
  }
  // Drop empty allowed_tenant_ids array — server treats absent ==
  // None, which means "no tenant restriction".
  if (Array.isArray(out.allowed_tenant_ids) && out.allowed_tenant_ids.length === 0) {
    delete out.allowed_tenant_ids
  }
  return out
}
