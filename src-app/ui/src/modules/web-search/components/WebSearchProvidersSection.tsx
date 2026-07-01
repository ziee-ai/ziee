import type { Resolver } from 'react-hook-form'
import { useEffect, useState } from 'react'
import {
  Button,
  Card,
  Separator,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  Input,
  PasswordInput,
  Spin,
  Text,
  Paragraph,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type ProviderCatalogEntry } from '@/api-client/types'

/**
 * Provider configuration, rendered GENERICALLY from the `GET /providers`
 * catalog: each descriptor's `config_fields` + (if it needs one) an API-key
 * input. Adding a backend provider surfaces its fields here with no change.
 *
 * Layout mirrors the code-sandbox / memory settings rhythm: one full-size
 * `<Card>` with each provider as a `<Separator titlePlacement="left">` section
 * (no nested `size="sm"` sub-cards, no status Tag in a card title).
 */
export function WebSearchProvidersSection() {
  const { providers, loading } = Stores.WebSearchAdmin
  if (loading && providers.length === 0) {
    return (
      <Card data-testid="websearch-providers-card" title="Search providers">
        <Spin label="Loading" />
      </Card>
    )
  }
  return (
    <Card data-testid="websearch-providers-card" title="Search providers">
      <Paragraph type="secondary" className="text-xs">
        Configure each engine you want available. Keys are stored encrypted and
        never shown again.
      </Paragraph>
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
  const [dirty, setDirty] = useState(false)

  const buildInit = (): ProviderFormValues => {
    const init: ProviderFormValues = { api_key: '' }
    for (const f of entry.config_fields) {
      init[f.key] =
        (entry.config as Record<string, string> | undefined)?.[f.key] ?? ''
    }
    return init
  }

  const schemaShape: Record<string, z.ZodTypeAny> = {
    api_key: z.string().optional(),
  }
  for (const f of entry.config_fields) {
    schemaShape[f.key] = f.required
      ? z.string().min(1, `${f.label} is required`)
      : z.string().optional()
  }
  const schema = z.object(schemaShape)

  const form = useForm<ProviderFormValues>({
    resolver: zodResolver(schema) as Resolver<ProviderFormValues>,
    defaultValues: buildInit(),
  })

  // Track user-driven edits (legacy `onValuesChange`): only a real `change`
  // marks the form dirty, NOT a programmatic reset/seed.
  useEffect(() => {
    const sub = form.watch((_, { type }) => {
      if (type === 'change') setDirty(true)
    })
    return () => sub.unsubscribe()
  }, [form])

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
      form.reset(buildInit())
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
      form.setValue('api_key', '')
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
      <Separator titlePlacement="left">
        <Text className="text-sm">{entry.display_name}</Text>
      </Separator>
      <Paragraph
        type={entry.configured ? 'success' : 'secondary'}
        className="text-xs !mb-2"
        data-testid={`websearch-provider-${entry.key}-status`}
        data-configured={entry.configured ? 'true' : 'false'}
      >
        {entry.configured ? 'Configured' : 'Not configured'}
      </Paragraph>

      <Form
        data-testid={`websearch-provider-${entry.key}-form`}
        form={form}
        layout="horizontal"
        onSubmit={onSubmit}
        disabled={!canManage}
      >
        {entry.config_fields.map(f => (
          <FormField key={f.key} name={f.key} label={f.label} required={f.required}>
            <Input data-testid={`websearch-provider-${entry.key}-field-${f.key}`} placeholder={f.placeholder} />
          </FormField>
        ))}

        {entry.needs_api_key && (
          <FormField
            name="api_key"
            label="API key"
            description={
              entry.api_key_set
                ? 'A key is stored. Leave blank to keep it, or type a new one to replace.'
                : 'No key stored yet.'
            }
          >
            <PasswordInput
              data-testid={`websearch-provider-${entry.key}-api-key`}
              showLabel="Show API key"
              hideLabel="Hide API key"
              autoComplete="new-password"
              placeholder={entry.api_key_set ? '•••••••• (stored)' : 'Enter API key'}
            />
          </FormField>
        )}

        <Flex justify="end" gap="small">
          {entry.needs_api_key && entry.api_key_set && (
            <Button data-testid={`websearch-provider-${entry.key}-clear`} variant="destructive" onClick={clearKey} disabled={!canManage || isSaving}>
              Clear key
            </Button>
          )}
          <Button
            data-testid={`websearch-provider-${entry.key}-save`}
            type="submit"
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
