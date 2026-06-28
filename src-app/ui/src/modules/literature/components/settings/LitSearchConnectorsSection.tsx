import { useEffect, useState } from 'react'
import {
  Button,
  Card,
  Divider,
  Flex,
  Form,
  Input,
  Spin,
  Switch,
  Tag,
  Typography,
  message,
} from 'antd'
import { Permissions, type ConnectorCatalogEntry } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Per-connector configuration, rendered GENERICALLY from the `GET /connectors`
 * catalog. Each connector: an enable toggle (membership in `enabled_connectors`),
 * its `config_fields` (e.g. Crossref mailto), and an optional/required API key.
 * Framing is "optional — raises limits" (all default sources work keyless),
 * except CORE whose key is required.
 */
export function LitSearchConnectorsSection() {
  const { connectors, loading } = Stores.LitSearchAdmin
  if (loading && connectors.length === 0) {
    return (
      <Card title="Sources">
        <Spin />
      </Card>
    )
  }
  return (
    <Card title="Sources">
      <Typography.Paragraph type="secondary" className="text-xs">
        Every default source works without a key. Optional keys only raise rate
        limits; CORE requires a free key. Keys are stored encrypted and never shown.
      </Typography.Paragraph>
      {connectors.map(c => (
        <ConnectorConfigForm key={c.key} entry={c} />
      ))}
    </Card>
  )
}

type FormValues = Record<string, string>

function ConnectorConfigForm({ entry }: { entry: ConnectorCatalogEntry }) {
  const canManage = usePermission(Permissions.LitSearchAdminManage)
  const { savingConnector, savingSettings, settings } = Stores.LitSearchAdmin
  const isSaving = savingConnector === entry.key
  const [form] = Form.useForm<FormValues>()
  const [dirty, setDirty] = useState(false)
  const apiKeyValue = Form.useWatch('api_key', form)

  // Re-seed when the stored config changes too (e.g. after a sibling save), so a
  // value the server returns is reflected and round-trips on the next save.
  const storedConfig = (entry.config ?? {}) as Record<string, unknown>
  const configKey = JSON.stringify({
    keys: entry.config_fields.map(f => f.key).sort(),
    vals: storedConfig,
  })
  useEffect(() => {
    if (!dirty) {
      // Pre-fill config fields from the STORED values (api_key stays blank —
      // it's write-only). Submitting empty fields would otherwise WIPE the
      // stored mailto/config on every save.
      const init: FormValues = { api_key: '' }
      for (const f of entry.config_fields) {
        const v = storedConfig[f.key]
        init[f.key] = typeof v === 'string' ? v : ''
      }
      form.setFieldsValue(init)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry.key, entry.api_key_set, configKey, form, dirty])

  const toggleEnabled = async (on: boolean) => {
    const current = settings?.enabled_connectors ?? []
    const next = on
      ? Array.from(new Set([...current, entry.key]))
      : current.filter(k => k !== entry.key)
    try {
      await Stores.LitSearchAdmin.updateSettings({ enabled_connectors: next })
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to toggle source')
    }
  }

  const onSubmit = async (v: FormValues) => {
    const config: Record<string, string> = {}
    for (const f of entry.config_fields) config[f.key] = (v[f.key] ?? '').trim()
    const body: { api_key?: string; config?: Record<string, string> } = { config }
    if (entry.key_field && v.api_key && v.api_key.trim().length > 0) {
      body.api_key = v.api_key.trim()
    }
    try {
      await Stores.LitSearchAdmin.updateConnector(entry.key, body)
      message.success(`${entry.display_name} saved`)
      form.setFieldValue('api_key', '')
      setDirty(false)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save source')
    }
  }

  const clearKey = async () => {
    try {
      await Stores.LitSearchAdmin.updateConnector(entry.key, { api_key: '' })
      message.success(`${entry.display_name} key cleared`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to clear key')
    }
  }

  const needsKey = entry.key_field?.required && !entry.api_key_set
  const hasFields = entry.config_fields.length > 0 || entry.key_field != null
  // Read the SAME field the toggle writes (`enabled_connectors`) so display and
  // mutation can't drift — `entry.enabled` (the catalog snapshot) is not
  // refreshed by `updateSettings`. Falls back to false until settings load.
  const isEnabled = settings?.enabled_connectors?.includes(entry.key) ?? false

  return (
    <>
      <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
        <Typography.Text className="text-sm">{entry.display_name}</Typography.Text>
      </Divider>
      <Typography.Paragraph type="secondary" className="text-xs !mb-1">
        {isEnabled && <Tag color="success">Active</Tag>}
        {needsKey && <Tag color="warning">Needs key</Tag>}
      </Typography.Paragraph>
      <Typography.Paragraph type="secondary" className="text-xs !mb-2">
        {entry.keyless_note}
      </Typography.Paragraph>

      <Flex align="center" gap="small" className="mb-2">
        <Switch
          aria-label={`Enable ${entry.display_name}`}
          checked={isEnabled}
          onChange={toggleEnabled}
          loading={savingSettings}
          // Disabled while a settings save is in flight → no double-toggle race.
          disabled={!canManage || !settings || savingSettings}
        />
        <Typography.Text className="text-xs">
          {isEnabled ? 'Enabled' : 'Disabled'}
        </Typography.Text>
      </Flex>

      {hasFields && (
        <Form
          form={form}
          name={`lit-connector-${entry.key}`}
          layout="horizontal"
          labelCol={{ xs: { span: 24 }, md: { span: 8 } }}
          wrapperCol={{ xs: { span: 24 }, md: { span: 16 } }}
          labelAlign="left"
          colon={false}
          onFinish={onSubmit}
          onValuesChange={() => setDirty(true)}
          disabled={!canManage}
        >
          {entry.config_fields.map(f => (
            <Form.Item
              key={f.key}
              name={f.key}
              label={f.label}
              extra={f.help ?? undefined}
              rules={f.required ? [{ required: true, message: `${f.label} is required` }] : []}
            >
              <Input placeholder={f.placeholder} />
            </Form.Item>
          ))}

          {entry.key_field && (
            <Form.Item
              name="api_key"
              label={entry.key_field.label}
              extra={
                <>
                  {entry.api_key_set
                    ? 'A key is stored. Leave blank to keep it, or type a new one.'
                    : (entry.key_field.help ?? 'No key stored yet.')}
                  {entry.key_field.docs_url && (
                    <>
                      {' '}
                      <a href={entry.key_field.docs_url} target="_blank" rel="noreferrer">
                        Get a key →
                      </a>
                    </>
                  )}
                </>
              }
              rules={
                entry.key_field.required && !entry.api_key_set
                  ? [{ required: true, message: `${entry.key_field.label} is required` }]
                  : []
              }
            >
              <Input.Password
                autoComplete="new-password"
                placeholder={entry.api_key_set ? '•••••••• (stored)' : 'Enter API key'}
              />
            </Form.Item>
          )}

          <Flex justify="end" gap="small">
            {entry.key_field && entry.api_key_set && (
              <Button danger onClick={clearKey} disabled={!canManage || isSaving}>
                Clear key
              </Button>
            )}
            <Button
              type="primary"
              htmlType="submit"
              loading={isSaving}
              disabled={!canManage || !dirty || (needsKey && !apiKeyValue?.trim())}
            >
              Save
            </Button>
          </Flex>
        </Form>
      )}
    </>
  )
}
