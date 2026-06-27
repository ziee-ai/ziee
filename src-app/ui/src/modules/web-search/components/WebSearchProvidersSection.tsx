import { useEffect, useState } from 'react'
import { Button, Card, Divider, Flex, Form, Input, Spin, Typography, message } from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type ProviderCatalogEntry } from '@/api-client/types'

/**
 * Provider configuration, rendered GENERICALLY from the `GET /providers`
 * catalog: each descriptor's `config_fields` + (if it needs one) an API-key
 * input. Adding a backend provider surfaces its fields here with no change.
 *
 * Layout mirrors the code-sandbox / memory settings rhythm: one full-size
 * `<Card>` with each provider as a `<Divider titlePlacement="start">` section
 * (no nested `size="small"` sub-cards, no status Tag in a card title).
 */
export function WebSearchProvidersSection() {
  const { providers, loading } = Stores.WebSearchAdmin
  if (loading && providers.length === 0) {
    return (
      <Card title="Search providers">
        <Spin />
      </Card>
    )
  }
  return (
    <Card title="Search providers">
      <Typography.Paragraph type="secondary" className="text-xs">
        Configure each engine you want available. Keys are stored encrypted and
        never shown again.
      </Typography.Paragraph>
      {providers.map(p => (
        <ProviderConfigForm key={p.key} entry={p} />
      ))}
    </Card>
  )
}

type ProviderFormValues = Record<string, string>

function ProviderConfigForm({ entry }: { entry: ProviderCatalogEntry }) {
  const canManage = usePermission(Permissions.WebSearchAdminManage)
  const { savingProvider } = Stores.WebSearchAdmin
  const isSaving = savingProvider === entry.key
  const [form] = Form.useForm<ProviderFormValues>()
  const [dirty, setDirty] = useState(false)

  // Seed config fields from stored config; api_key stays blank (write-only).
  // Depend on this provider's OWN identity + stored state — NOT the whole
  // `entry` object reference. The store replaces the entire providers array on
  // every save, so depending on `entry` would re-seed (and wipe unsaved edits
  // in) EVERY form whenever ANY provider is saved. Keyed this way, a sibling
  // save leaves this form's in-progress edits intact.
  const configKey = JSON.stringify(entry.config ?? {})
  useEffect(() => {
    // Only re-seed when there are no unsaved edits — so a same-form side-channel
    // (e.g. clearKey flipping this provider's api_key_set) can't wipe in-progress
    // config-field edits.
    if (!dirty) {
      const init: ProviderFormValues = { api_key: '' }
      for (const f of entry.config_fields) {
        init[f.key] =
          (entry.config as Record<string, string> | undefined)?.[f.key] ?? ''
      }
      form.setFieldsValue(init)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry.key, entry.api_key_set, configKey, form, dirty])

  const onSubmit = async (v: ProviderFormValues) => {
    const config: Record<string, string> = {}
    for (const f of entry.config_fields) {
      config[f.key] = (v[f.key] ?? '').trim()
    }
    const body: { api_key?: string; config?: Record<string, string> } = {
      config,
    }
    // Only send api_key when the admin typed one (blank = keep existing).
    if (entry.needs_api_key && v.api_key && v.api_key.trim().length > 0) {
      body.api_key = v.api_key.trim()
    }
    try {
      await Stores.WebSearchAdmin.updateProvider(entry.key, body)
      message.success(`${entry.display_name} saved`)
      form.setFieldValue('api_key', '')
      setDirty(false)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save provider')
    }
  }

  const clearKey = async () => {
    try {
      await Stores.WebSearchAdmin.updateProvider(entry.key, { api_key: '' })
      message.success(`${entry.display_name} API key cleared`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to clear key')
    }
  }

  return (
    <>
      <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
        <Typography.Text className="text-sm">{entry.display_name}</Typography.Text>
      </Divider>
      <Typography.Paragraph
        type={entry.configured ? 'success' : 'secondary'}
        className="text-xs !mb-2"
      >
        {entry.configured ? 'Configured' : 'Not configured'}
      </Typography.Paragraph>

      <Form
        form={form}
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
            rules={
              f.required
                ? [{ required: true, message: `${f.label} is required` }]
                : []
            }
          >
            <Input placeholder={f.placeholder} />
          </Form.Item>
        ))}

        {entry.needs_api_key && (
          <Form.Item
            name="api_key"
            label="API key"
            extra={
              entry.api_key_set
                ? 'A key is stored. Leave blank to keep it, or type a new one to replace.'
                : 'No key stored yet.'
            }
          >
            <Input.Password
              autoComplete="new-password"
              placeholder={entry.api_key_set ? '•••••••• (stored)' : 'Enter API key'}
            />
          </Form.Item>
        )}

        <Flex justify="end" gap="small">
          {entry.needs_api_key && entry.api_key_set && (
            <Button danger onClick={clearKey} disabled={!canManage || isSaving}>
              Clear key
            </Button>
          )}
          <Button
            type="primary"
            htmlType="submit"
            loading={isSaving}
            disabled={!canManage || !dirty}
          >
            Save
          </Button>
        </Flex>
      </Form>
    </>
  )
}
