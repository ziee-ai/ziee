import { useEffect, useRef, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Empty,
  Flex,
  Form,
  FormField,
  Paragraph,
  PasswordInput,
  Separator,
  Spin,
  Tag,
  Text,
  message,
  useForm,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

/**
 * User-facing page: the caller's OWN lit-search connector keys. Mirrors the
 * admin connector form, but scoped to the current user — one masked key-entry
 * row per key-accepting connector, resolved before the shared deployment key.
 * Registered in the `settingsUserPages` slot, gated on `lit_search::use`.
 */
export function LitSearchUserKeysPage() {
  const { connectors, loading, error, savingConnector } = Stores.LitSearchUserKeys

  const buildDefaults = () => {
    const out: Record<string, string> = {}
    for (const c of connectors) out[c.connector] = ''
    return out
  }
  const form = useForm<Record<string, string>>({ defaultValues: buildDefaults() })
  const [saving, setSaving] = useState(false)

  const sig = JSON.stringify(connectors.map(c => [c.connector, c.system_key_set, c.user_key]))
  const lastSig = useRef(sig)
  useEffect(() => {
    if (sig !== lastSig.current || !form.formState.isDirty) {
      lastSig.current = sig
      form.reset(buildDefaults())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig])

  const onSaveAll = async (values: Record<string, string>) => {
    setSaving(true)
    try {
      let saved = 0
      for (const c of connectors) {
        const key = (values[c.connector] ?? '').trim()
        if (key.length === 0) continue
        await Stores.LitSearchUserKeys.saveKey(c.connector, key)
        saved += 1
      }
      if (saved > 0) {
        message.success('Your API key was saved')
        form.reset(buildDefaults())
      }
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save your key')
    } finally {
      setSaving(false)
    }
  }

  const clearKey = async (connector: string, displayName: string) => {
    try {
      await Stores.LitSearchUserKeys.clearKey(connector)
      message.success(`Your ${displayName} key was removed`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to remove key')
    }
  }

  return (
    <SettingsPageContainer
      title="Literature Keys"
      subtitle="Set your own API keys for literature-search sources. Your key is used before the shared deployment key; leave a source blank to use the shared key."
    >
      {error && (
        <Alert
          data-testid="litsearch-user-keys-error-alert"
          tone="error"
          title="Failed to load your literature keys"
          description={error}
          className="mb-3"
        />
      )}

      <Card
        data-testid="litsearch-user-keys-card"
        title="Your connector keys"
        footer={
          connectors.length > 0 ? (
            <SettingsFormActions
              onSave={form.handleSubmit(onSaveAll)}
              onCancel={() => form.reset(buildDefaults())}
              saving={saving || savingConnector !== null}
              saveTestid="litsearch-user-keys-save"
              cancelTestid="litsearch-user-keys-cancel"
            />
          ) : undefined
        }
      >
        {loading && connectors.length === 0 ? (
          <div className="flex justify-center py-8">
            <Spin label="Loading your keys" />
          </div>
        ) : connectors.length === 0 ? (
          <Empty
            data-testid="litsearch-user-keys-empty"
            description="No literature sources accept a personal API key on this deployment."
          />
        ) : (
          <>
            <Paragraph type="secondary" className="text-sm">
              Keys are stored encrypted and never shown again. Type a new value to
              replace an existing key.
            </Paragraph>
            <Form
              data-testid="litsearch-user-keys-form"
              form={form}
              layout="horizontal"
              onSubmit={onSaveAll}
            >
              {connectors.map(entry => (
                <div key={entry.connector}>
                  <Separator titlePlacement="left" className="mt-5 mb-3">
                    <Text className="text-sm">{entry.display_name}</Text>
                  </Separator>

                  <Flex align="center" gap="small" className="mb-2">
                    {entry.user_key ? (
                      <Tag tone="success" data-testid={`litsearch-user-key-${entry.connector}-status`}>
                        Using your key ({entry.user_key})
                      </Tag>
                    ) : entry.system_key_set ? (
                      <Tag data-testid={`litsearch-user-key-${entry.connector}-status`}>
                        Using shared key
                      </Tag>
                    ) : (
                      <Tag data-testid={`litsearch-user-key-${entry.connector}-status`}>
                        Keyless (or add your own)
                      </Tag>
                    )}
                  </Flex>

                  <FormField
                    name={entry.connector}
                    label={entry.key_field?.label ?? 'Your API key'}
                    description={
                      entry.key_field?.help ??
                      (entry.user_key
                        ? 'Your key is stored. Leave blank to keep it, or type a new one to replace.'
                        : 'Optional — leave blank to use the shared key (or keyless access).')
                    }
                  >
                    <PasswordInput
                      data-testid={`litsearch-user-key-${entry.connector}-input`}
                      showLabel="Show API key"
                      hideLabel="Hide API key"
                      autoComplete="new-password"
                      placeholder={entry.user_key ? '•••••••• (stored)' : 'Enter API key'}
                    />
                  </FormField>

                  {entry.key_field?.docs_url && (
                    <Paragraph type="secondary" className="text-xs mb-1">
                      <a href={entry.key_field.docs_url} target="_blank" rel="noreferrer">
                        Where do I get a key?
                      </a>
                    </Paragraph>
                  )}

                  {entry.user_key && (
                    <Flex justify="end" className="mb-2">
                      <Button
                        data-testid={`litsearch-user-key-${entry.connector}-clear`}
                        variant="destructive"
                        onClick={() => clearKey(entry.connector, entry.display_name)}
                        disabled={savingConnector !== null}
                      >
                        Remove your key
                      </Button>
                    </Flex>
                  )}
                </div>
              ))}
            </Form>
          </>
        )}
      </Card>
    </SettingsPageContainer>
  )
}
