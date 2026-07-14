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
  Tag,
  Text,
  Paragraph,
  message,
} from '@ziee/kit'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

/**
 * Provider configuration, rendered GENERICALLY from the `GET /providers`
 * catalog. All providers share ONE form (values namespaced by provider key) and
 * ONE Save button in the card footer that persists every provider at once.
 */
export function WebSearchProvidersSection() {
  const { providers, loading } = Stores.WebSearchAdmin
  const canManage = usePermission(Permissions.WebSearchAdminManage)

  // Nested defaults: { <providerKey>: { <field>: '', api_key: '' } }.
  const buildDefaults = () => {
    const out: Record<string, Record<string, string>> = {}
    for (const p of providers) {
      const one: Record<string, string> = { api_key: '' }
      for (const f of p.config_fields) {
        one[f.key] = (p.config as Record<string, string> | undefined)?.[f.key] ?? ''
      }
      out[p.key] = one
    }
    return out
  }

  const form = useForm<Record<string, Record<string, string>>>({ defaultValues: buildDefaults() })
  const [saving, setSaving] = useState(false)

  // Re-seed only when the underlying catalog data changes AND there are no
  // unsaved edits (a save replaces the providers array, which would otherwise
  // wipe in-progress edits).
  const sig = JSON.stringify(providers.map((p) => [p.key, p.config, p.api_key_set]))
  const lastSig = useRef(sig)
  useEffect(() => {
    if (sig !== lastSig.current || !form.formState.isDirty) {
      lastSig.current = sig
      form.reset(buildDefaults())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig])

  const onSaveAll = async (values: Record<string, Record<string, string>>) => {
    setSaving(true)
    try {
      for (const p of providers) {
        const v = values[p.key] ?? {}
        const config: Record<string, string> = {}
        for (const f of p.config_fields) config[f.key] = (v[f.key] ?? '').trim()
        const body: { api_key?: string; config?: Record<string, string> } = { config }
        if (p.needs_api_key && v.api_key && v.api_key.trim().length > 0) {
          body.api_key = v.api_key.trim()
        }
        await Stores.WebSearchAdmin.updateProvider(p.key, body)
      }
      message.success('Search providers saved')
      form.reset(buildDefaults()) // clears the api_key inputs
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save providers')
    } finally {
      setSaving(false)
    }
  }

  const clearKey = async (providerKey: string, displayName: string) => {
    try {
      await Stores.WebSearchAdmin.updateProvider(providerKey, { api_key: '' })
      message.success(`${displayName} API key cleared`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to clear key')
    }
  }

  if (loading && providers.length === 0) {
    return (
      <Card data-testid="websearch-providers-card" title="Search providers">
        <Spin label="Loading" />
      </Card>
    )
  }

  return (
    <Card
      data-testid="websearch-providers-card"
      title="Search providers"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(onSaveAll)}
          onCancel={() => form.reset(buildDefaults())}
          saving={saving}
          saveTestid="websearch-providers-save"
          cancelTestid="websearch-providers-cancel"
        />
      ) : undefined}
    >
      <Paragraph type="secondary" className="text-sm">
        Configure each engine you want available. Keys are stored encrypted and
        never shown again.
      </Paragraph>

      <Form
        data-testid="websearch-providers-form"
        form={form}
        layout="horizontal"
        onSubmit={onSaveAll}
        disabled={!canManage}
      >
        {providers.map((entry) => (
          <div key={entry.key}>
            <Separator titlePlacement="left" className="mt-5 mb-3">
              <Text className="text-sm">{entry.display_name}</Text>
            </Separator>

            <div className="mb-3">
              <Flex align="center" gap="small" className="mb-1">
                <Tag
                  tone={entry.configured ? 'success' : 'warning'}
                  data-testid={`websearch-provider-${entry.key}-status`}
                  data-configured={String(entry.configured)}
                >
                  {entry.configured
                    ? entry.needs_api_key
                      ? 'Shared key set'
                      : 'Configured'
                    : entry.needs_api_key
                      ? 'No key set'
                      : 'Not configured'}
                </Tag>
              </Flex>
              <Text type="secondary" className="text-xs">
                {entry.configured
                  ? entry.needs_api_key
                    ? 'A shared deployment key is set — every user without their own key searches through it. Users can still add a personal key to use their own quota and higher rate limits.'
                    : 'This provider is configured and available to all users.'
                  : entry.needs_api_key
                    ? 'No shared key is set, so this provider is unavailable to users who have not added their own key. Set a deployment key below to enable it for everyone; a paid plan raises the shared rate limit.'
                    : 'Complete this provider’s configuration below before it can be used.'}
              </Text>
            </div>

            {entry.config_fields.map((f) => (
              <FormField key={f.key} name={`${entry.key}.${f.key}`} label={f.label} required={f.required}>
                <Input data-testid={`websearch-provider-${entry.key}-field-${f.key}`} placeholder={f.placeholder} />
              </FormField>
            ))}

            {entry.needs_api_key && (
              <FormField
                name={`${entry.key}.api_key`}
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

            {entry.needs_api_key && entry.api_key_set && (
              <Flex justify="end" className="mb-2">
                <Button
                  data-testid={`websearch-provider-${entry.key}-clear`}
                  variant="destructive"
                  onClick={() => clearKey(entry.key, entry.display_name)}
                  disabled={!canManage || saving}
                >
                  Clear key
                </Button>
              </Flex>
            )}
          </div>
        ))}
      </Form>
    </Card>
  )
}
