import type { Resolver } from 'react-hook-form'
import { useEffect, useMemo } from 'react'
import {
  Button,
  Card,
  Flex,
  Form,
  FormField,
  Input,
  PasswordInput,
  Paragraph,
  Separator,
  Spin,
  Switch,
  Tag,
  Text,
  message,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
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
      <Card title="Sources" data-testid="lit-connectors-card">
        <Spin label="Loading" />
      </Card>
    )
  }
  return (
    <Card title="Sources" data-testid="lit-connectors-card">
      <Paragraph type="secondary" className="text-xs">
        Every default source works without a key. Optional keys only raise rate
        limits; CORE requires a free key. Keys are stored encrypted and never shown.
      </Paragraph>
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

  // Re-seed when the stored config changes too (e.g. after a sibling save), so a
  // value the server returns is reflected and round-trips on the next save.
  const storedConfig = (entry.config ?? {}) as Record<string, unknown>
  const configKey = JSON.stringify({
    keys: entry.config_fields.map(f => f.key).sort(),
    vals: storedConfig,
  })

  // Pre-fill config fields from the STORED values (api_key stays blank — it's
  // write-only). Submitting empty fields would otherwise WIPE the stored
  // mailto/config on every save.
  const buildInit = (): FormValues => {
    const init: FormValues = { api_key: '' }
    for (const f of entry.config_fields) {
      const v = storedConfig[f.key]
      init[f.key] = typeof v === 'string' ? v : ''
    }
    return init
  }

  const schema = useMemo(() => {
    const shape: Record<string, z.ZodTypeAny> = { api_key: z.string().optional() }
    for (const f of entry.config_fields) {
      shape[f.key] = f.required
        ? z.string().min(1, `${f.label} is required`)
        : z.string().optional()
    }
    if (entry.key_field?.required && !entry.api_key_set) {
      shape.api_key = z.string().min(1, `${entry.key_field.label} is required`)
    }
    return z.object(shape)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry.key, entry.api_key_set, configKey])

  const form = useForm<FormValues>({
    resolver: zodResolver(schema) as Resolver<FormValues>,
    defaultValues: buildInit(),
  })
  const dirty = form.formState.isDirty
  const apiKeyValue = form.watch('api_key')

  useEffect(() => {
    if (!form.formState.isDirty) {
      form.reset(buildInit())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entry.key, entry.api_key_set, configKey, form])

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
      form.reset({ ...v, api_key: '' })
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
      <Separator titlePlacement="left">
        <Text className="text-sm">{entry.display_name}</Text>
      </Separator>
      <Paragraph type="secondary" className="text-xs !mb-1">
        {isEnabled && <Tag tone="success" data-testid={`lit-connector-active-tag-${entry.key}`}>Active</Tag>}
        {needsKey && <Tag tone="warning" data-testid={`lit-connector-needs-key-tag-${entry.key}`}>Needs key</Tag>}
      </Paragraph>
      <Paragraph type="secondary" className="text-xs !mb-2">
        {entry.keyless_note}
      </Paragraph>

      <Flex align="center" gap="small" className="mb-2">
        <Switch
          aria-label={`Enable ${entry.display_name}`}
          checked={isEnabled}
          onChange={toggleEnabled}
          loading={savingSettings}
          // Disabled while a settings save is in flight → no double-toggle race.
          disabled={!canManage || !settings || savingSettings}
          data-testid={`lit-connector-enable-switch-${entry.key}`}
        />
        <Text className="text-xs">{isEnabled ? 'Enabled' : 'Disabled'}</Text>
      </Flex>

      {hasFields && (
        <Form
          form={form}
          name={`lit-connector-${entry.key}`}
          layout="horizontal"
          onSubmit={onSubmit}
          disabled={!canManage}
          data-testid={`lit-connector-form-${entry.key}`}
        >
          {entry.config_fields.map(f => (
            <FormField
              key={f.key}
              name={f.key}
              label={f.label}
              description={f.help ?? undefined}
              required={f.required}
            >
              <Input placeholder={f.placeholder} data-testid={`lit-connector-config-input-${entry.key}-${f.key}`} />
            </FormField>
          ))}

          {entry.key_field && (
            <FormField
              name="api_key"
              label={entry.key_field.label}
              required={entry.key_field.required && !entry.api_key_set}
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

          <Flex justify="end" gap="small">
            {entry.key_field && entry.api_key_set && (
              <Button
                variant="destructive"
                onClick={clearKey}
                disabled={!canManage || isSaving}
                data-testid={`lit-connector-clear-key-button-${entry.key}`}
              >
                Clear key
              </Button>
            )}
            <Button
              type="submit"
              loading={isSaving}
              disabled={!canManage || !dirty || (needsKey && !apiKeyValue?.trim())}
              data-testid={`lit-connector-save-button-${entry.key}`}
            >
              Save
            </Button>
          </Flex>
        </Form>
      )}
    </>
  )
}
