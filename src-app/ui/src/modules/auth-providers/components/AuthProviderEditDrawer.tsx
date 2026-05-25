import { useEffect, useMemo } from 'react'
import {
  Alert,
  Button,
  Drawer,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Typography,
} from 'antd'
import { Permissions } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions/Can'
import { usePermission } from '@/core/permissions/usePermission'
import type { AuthProviderResponse } from '@/api-client/types'
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
  const [form] = Form.useForm<FormShape>()
  const { saving, error } = Stores.AuthProvidersAdmin
  const canManage = usePermission(Permissions.AuthProvidersManage)

  // Resolve the provider type for this drawer instance.
  const providerType = useMemo<string>(
    () => template?.provider_type ?? existing?.provider_type ?? 'oidc',
    [template, existing],
  )

  useEffect(() => {
    if (!open) return
    if (template) {
      form.setFieldsValue({
        name: '',
        enabled: true,
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
      } else if (existing) {
        await Stores.AuthProvidersAdmin.updateProvider(existing.id, {
          name: values.name.trim(),
          enabled: values.enabled,
          config: normalized,
        })
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
      width={560}
      destroyOnClose
      footer={
        <Space className="flex justify-end">
          <Button onClick={onClose}>Cancel</Button>
          <Can permission={Permissions.AuthProvidersManage}>
            <Button type="primary" loading={saving} onClick={onSubmit}>
              {existing ? 'Save' : 'Create'}
            </Button>
          </Can>
        </Space>
      }
    >
      {error && (
        <Alert type="error" message={error} showIcon className="mb-4" />
      )}
      <Form form={form} layout="vertical" disabled={!canManage}>
        <Form.Item
          name="name"
          label="Name (URL slug)"
          rules={[
            { required: true, message: 'Provider name required' },
            {
              pattern: /^[a-z0-9-]+$/i,
              message:
                'Use only letters, digits, and hyphens (appears in URLs)',
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
      </Form>
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
