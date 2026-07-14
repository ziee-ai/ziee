import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Card,
  Empty,
  ErrorState,
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
} from '@ziee/kit'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

/**
 * User-facing page: the caller's OWN web-search provider keys. Mirrors the
 * admin `WebSearchProvidersSection` form pattern, but scoped to the current
 * user — one masked key-entry row per key-accepting provider, resolved before
 * the shared deployment key. Registered in the `settingsUserPages` slot, gated
 * on `web_search::use`.
 */
export function WebSearchUserKeysPage() {
  const { providers, loading, error, savingProvider } = Stores.WebSearchUserKeys

  const buildDefaults = () => {
    const out: Record<string, string> = {}
    for (const p of providers) out[p.provider] = ''
    return out
  }
  const form = useForm<Record<string, string>>({ defaultValues: buildDefaults() })
  const [saving, setSaving] = useState(false)

  const sig = JSON.stringify(providers.map(p => [p.provider, p.system_key_set, p.user_key]))
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
      for (const p of providers) {
        const key = (values[p.provider] ?? '').trim()
        if (key.length === 0) continue
        await Stores.WebSearchUserKeys.saveKey(p.provider, key)
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

  const clearKey = async (provider: string, displayName: string) => {
    try {
      await Stores.WebSearchUserKeys.clearKey(provider)
      message.success(`Your ${displayName} key was removed`)
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to remove key')
    }
  }

  return (
    <SettingsPageContainer
      title="Web Search Keys"
      subtitle="Set your own API keys for web-search providers. Your key is used before the shared deployment key; leave a provider blank to use the shared key."
    >
      <Card
        data-testid="websearch-user-keys-card"
        title="Your provider keys"
        footer={
          providers.length > 0 ? (
            <SettingsFormActions
              onSave={form.handleSubmit(onSaveAll)}
              onCancel={() => form.reset(buildDefaults())}
              saving={saving || savingProvider !== null}
              saveTestid="websearch-user-keys-save"
              cancelTestid="websearch-user-keys-cancel"
            />
          ) : undefined
        }
      >
        {loading && providers.length === 0 ? (
          <div className="flex justify-center py-8">
            <Spin label="Loading your keys" />
          </div>
        ) : providers.length === 0 ? (
          error ? (
            <ErrorState
              resource="your web search keys"
              description="Your provider keys couldn't be loaded. Check your connection and try again."
              details={error}
              onRetry={() => void Stores.WebSearchUserKeys.load()}
              data-testid="websearch-user-keys-error"
            />
          ) : (
            <Empty
              data-testid="websearch-user-keys-empty"
              description="No web-search providers accept a personal API key on this deployment."
            />
          )
        ) : (
          <>
            <Paragraph type="secondary" className="text-sm">
              Keys are stored encrypted and never shown again. Type a new value to
              replace an existing key.
            </Paragraph>
            <Form
              data-testid="websearch-user-keys-form"
              form={form}
              layout="horizontal"
              onSubmit={onSaveAll}
            >
              {providers.map(entry => (
                <div key={entry.provider}>
                  <Separator titlePlacement="left" className="mt-5 mb-3">
                    <Text className="text-sm">{entry.display_name}</Text>
                  </Separator>

                  <div className="mb-2">
                    <Flex align="center" gap="small" className="mb-1">
                      {entry.user_key ? (
                        <Tag tone="success" data-testid={`websearch-user-key-${entry.provider}-status`}>
                          Using your key ({entry.user_key})
                        </Tag>
                      ) : entry.system_key_set ? (
                        <Tag tone="info" data-testid={`websearch-user-key-${entry.provider}-status`}>
                          Shared key set by admin
                        </Tag>
                      ) : (
                        <Tag tone="warning" data-testid={`websearch-user-key-${entry.provider}-status`}>
                          No key set
                        </Tag>
                      )}
                    </Flex>
                    <Text type="secondary" className="text-xs">
                      {entry.user_key
                        ? 'Your key is used before the shared deployment key and draws on your own quota — usually higher rate limits.'
                        : entry.system_key_set
                          ? 'Your administrator has set a shared key, so this provider works now. Add your own key below to use your personal quota and higher rate limits instead of the shared one.'
                          : 'Your administrator has not set a key, so this provider is unavailable until you add your own key below.'}
                    </Text>
                  </div>

                  <FormField
                    name={entry.provider}
                    label="Your API key"
                    description={
                      entry.user_key
                        ? 'Your key is stored. Leave blank to keep it, or type a new one to replace.'
                        : entry.system_key_set
                          ? 'Optional — leave blank to keep using the shared deployment key.'
                          : 'No shared key is set; enter your own to use this provider.'
                    }
                  >
                    <PasswordInput
                      data-testid={`websearch-user-key-${entry.provider}-input`}
                      showLabel="Show API key"
                      hideLabel="Hide API key"
                      autoComplete="new-password"
                      placeholder={entry.user_key ? '•••••••• (stored)' : 'Enter API key'}
                    />
                  </FormField>

                  {entry.user_key && (
                    <Flex justify="end" className="mb-2">
                      <Button
                        data-testid={`websearch-user-key-${entry.provider}-clear`}
                        variant="destructive"
                        onClick={() => clearKey(entry.provider, entry.display_name)}
                        disabled={savingProvider !== null}
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
