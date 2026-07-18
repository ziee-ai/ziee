import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Card,
  Separator,
  Flex,
  Form,
  FormField,
  useForm,
  Input,
  PasswordInput,
  Spin,
  Text,
  Paragraph,
  Switch,
  Tag,
  message,
} from '@ziee/kit'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

/**
 * Per-connector configuration, rendered GENERICALLY from the `GET /connectors`
 * catalog. All sources share ONE form (values namespaced by connector key) and
 * ONE Save button in the card footer that persists every source at once —
 * enable toggles, config fields and API keys.
 */
export function LitSearchConnectorsSection() {
  const { connectors, loading, settings } = Stores.LitSearchAdmin
  const canManage = usePermission(Permissions.LitSearchAdminManage)
  const [saving, setSaving] = useState(false)

  const buildDefaults = () => {
    const enabled = new Set(settings?.enabled_connectors ?? [])
    const out: Record<string, Record<string, string | boolean>> = {}
    for (const c of connectors) {
      const one: Record<string, string | boolean> = { enabled: enabled.has(c.key), api_key: '' }
      const cfg = (c.config ?? {}) as Record<string, unknown>
      for (const f of c.config_fields) one[f.key] = typeof cfg[f.key] === 'string' ? (cfg[f.key] as string) : ''
      out[c.key] = one
    }
    return out
  }

  const form = useForm<Record<string, Record<string, string | boolean>>>({ defaultValues: buildDefaults() })

  const sig = JSON.stringify([
    settings?.enabled_connectors,
    connectors.map((c) => [c.key, c.config, c.api_key_set]),
  ])
  const lastSig = useRef(sig)
  useEffect(() => {
    if (sig !== lastSig.current || !form.formState.isDirty) {
      lastSig.current = sig
      form.reset(buildDefaults())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig])

  const onSaveAll = async (values: Record<string, Record<string, string | boolean>>) => {
    setSaving(true)
    try {
      const enabled = connectors.filter((c) => values[c.key]?.enabled).map((c) => c.key)
      await Stores.LitSearchAdmin.updateSettings({ enabled_connectors: enabled })
      for (const c of connectors) {
        if (c.config_fields.length === 0 && !c.key_field) continue
        const v = values[c.key] ?? {}
        const config: Record<string, string> = {}
        for (const f of c.config_fields) config[f.key] = String(v[f.key] ?? '').trim()
        const body: { api_key?: string; config?: Record<string, string> } = { config }
        const apiKey = String(v.api_key ?? '').trim()
        if (c.key_field && apiKey) body.api_key = apiKey
        await Stores.LitSearchAdmin.updateConnector(c.key, body)
      }
      message.success('Sources saved')
      form.reset(buildDefaults())
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save sources')
    } finally {
      setSaving(false)
    }
  }

  const clearKey = async (key: string, name: string) => {
    try {
      await Stores.LitSearchAdmin.updateConnector(key, { api_key: '' })
      message.success(`${name} key cleared`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to clear key')
    }
  }

  if (loading && connectors.length === 0) {
    return (
      <Card title="Sources" data-testid="lit-connectors-card">
        <Spin label="Loading" />
      </Card>
    )
  }

  return (
    <Card
      title="Sources"
      data-testid="lit-connectors-card"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(onSaveAll)}
          onCancel={() => form.reset(buildDefaults())}
          saving={saving}
          saveTestid="lit-connectors-save"
          cancelTestid="lit-connectors-cancel"
        />
      ) : undefined}
    >
      <Paragraph type="secondary" className="text-sm">
        Every default source works without a key. Optional keys only raise rate
        limits; CORE requires a free key. Keys are stored encrypted and never shown.
      </Paragraph>

      <Form
        form={form}
        layout="horizontal"
        onSubmit={onSaveAll}
        disabled={!canManage}
        data-testid="lit-connectors-form"
      >
        {connectors.map((entry) => {
          return (
            <div key={entry.key}>
              <Separator titlePlacement="left" className="mt-5 mb-3">
                <Text className="text-sm">{entry.display_name}</Text>
              </Separator>
              {entry.key_field && (
                <div className="mb-2">
                  <Flex align="center" gap="small" className="mb-1">
                    {entry.api_key_set ? (
                      <Tag tone="success" data-testid={`lit-connector-key-status-${entry.key}`}>
                        Shared key set
                      </Tag>
                    ) : entry.key_field.required ? (
                      <Tag variant="outline" tone="warning" data-testid={`lit-connector-needs-key-tag-${entry.key}`}>
                        Needs key
                      </Tag>
                    ) : (
                      <Tag tone="info" data-testid={`lit-connector-key-status-${entry.key}`}>
                        No key — keyless
                      </Tag>
                    )}
                  </Flex>
                  <Text type="secondary" className="text-xs">
                    {entry.api_key_set
                      ? 'This shared key is used for every user who hasn’t set their own. Users can override it with a personal key to use their own quota and higher rate limits.'
                      : entry.key_field.required
                        ? 'This source requires a key and stays unavailable until you set one here (or a user adds their own).'
                        : 'This source works without a key, subject to public rate limits. Set a key here to raise limits for all users; individual users can also add their own.'}
                  </Text>
                </div>
              )}
              {entry.keyless_note && (
                <Paragraph type="secondary" className="!mb-2">
                  {entry.keyless_note}
                </Paragraph>
              )}

              <div className="flex flex-col gap-5">
                <FormField name={`${entry.key}.enabled`} label="Enable" valuePropName="checked">
                  <Switch tooltip={`Enable ${entry.display_name}`} data-testid={`lit-connector-enable-switch-${entry.key}`} />
                </FormField>

                {entry.config_fields.map((f) => (
                  <FormField
                    key={f.key}
                    name={`${entry.key}.${f.key}`}
                    label={f.label}
                    description={f.help ?? undefined}
                    required={f.required}
                  >
                    <Input placeholder={f.placeholder} data-testid={`lit-connector-config-input-${entry.key}-${f.key}`} />
                  </FormField>
                ))}

                {entry.key_field && (
                  <FormField
                    name={`${entry.key}.api_key`}
                    label={entry.key_field.label}
                    description={
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
                  >
                    <PasswordInput
                      showLabel="Show key"
                      hideLabel="Hide key"
                      placeholder={entry.api_key_set ? '•••••••• (stored)' : 'Enter API key'}
                      data-testid={`lit-connector-api-key-input-${entry.key}`}
                    />
                  </FormField>
                )}
              </div>

              {entry.key_field && entry.api_key_set && (
                <Flex justify="end" className="mb-2">
                  <Button
                    variant="destructive"
                    onClick={() => clearKey(entry.key, entry.display_name)}
                    disabled={!canManage || saving}
                    data-testid={`lit-connector-clear-key-button-${entry.key}`}
                  >
                    Clear key
                  </Button>
                </Flex>
              )}
            </div>
          )
        })}
      </Form>
    </Card>
  )
}
