import {
  Button,
  Card,
  Flex,
  Form,
  FormField,
  Input,
  PasswordInput,
  Separator,
  Text,
  Title,
  message,
  useForm,
} from '@/components/ui'
import { useEffect } from 'react'
import { useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { ProviderProxySettingsForm } from '@/modules/llm-provider/components/ProviderProxySettingsForm'
import { ProviderHeader } from '@/modules/llm-provider/components/ProviderHeader'
import { LlmModelsSection } from '@/modules/llm-provider/components/LlmModelsSection'
import { ProviderGroupAssignmentCard } from '@/modules/llm-provider/components/ProviderGroupAssignmentCard'
import { AddRemoteLlmModelDrawer } from '@/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer'
import { EditLlmModelDrawer } from '@/modules/llm-provider/components/llm-models/EditLlmModelDrawer'
import type { ProxySettings } from '@/api-client/types'

interface RemoteProviderFormValues {
  api_key?: string
  base_url?: string
}

export function RemoteProviderSettings() {
  const { providerId } = useParams<{ providerId?: string }>()

  const form = useForm<RemoteProviderFormValues>({
    defaultValues: { api_key: '', base_url: '' },
  })

  // Store data
  const { error } = Stores.LlmProvider
  const canEditProvider = usePermission(Permissions.LlmProvidersEdit)

  // Get current provider and its models
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === providerId,
  )

  const isDirty = form.formState.isDirty

  const handleSaveSettings = async (values: RemoteProviderFormValues) => {
    if (!currentProvider) return

    // Only send fields the user actually changed. The api_key field is
    // write-only (server never returns it); sending an empty/unchanged value
    // must not clobber the stored key.
    const dirty = form.formState.dirtyFields
    const pendingSettings: RemoteProviderFormValues = {}
    if (dirty.api_key) pendingSettings.api_key = values.api_key
    if (dirty.base_url) pendingSettings.base_url = values.base_url

    if (Object.keys(pendingSettings).length === 0) return

    try {
      await Stores.LlmProvider.updateLlmProvider(
        currentProvider.id,
        pendingSettings,
      )

      // Reset dirty state to the just-saved values.
      form.reset(values)
      message.success('Settings saved')
    } catch (error) {
      console.error('Failed to save settings:', error)
      // Error is handled by the store
    }
  }

  const handleProxySettingsSave = async (proxySettings: any) => {
    if (!currentProvider) return

    try {
      await Stores.LlmProvider.updateLlmProvider(currentProvider.id, {
        proxy_settings: proxySettings,
      })
      message.success('Proxy settings saved')
    } catch (error) {
      console.error('Failed to save proxy settings:', error)
      // Error is handled by the store
    }
  }

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.LlmProvider.clearLlmProviderStoreError()
    }
  }, [error])

  // Initialise / re-initialise the form ONLY when the user navigates to
  // a different provider (the id changes). Previously this depended on
  // `currentProvider` itself — a recomputed object reference from
  // `providers.find(...)` on every render — so any unrelated store
  // mutation (e.g., an SSE update from another tab) would re-fire this
  // effect and overwrite the user's mid-edit input. Guarded by the
  // explicit `providerId` from useParams instead.
  useEffect(() => {
    if (!currentProvider) return
    form.reset({
      api_key: currentProvider.api_key ?? '',
      base_url: currentProvider.base_url ?? '',
    })
    // Intentionally exclude `currentProvider` from deps — re-init is
    // keyed on provider-id, not on any store mutation that produces a
    // new object reference. Reading `currentProvider` here is safe
    // because we just confirmed the id matches.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId, form])

  // Return early if no provider or not remote
  if (!currentProvider || currentProvider.provider_type === 'local') {
    return null
  }

  return (
    <Flex className={'flex-col gap-3'}>
      <ProviderHeader />

      {/* API Configuration */}
      <Form
        name="remote-provider-settings-form"
        form={form}
        layout="vertical"
        onSubmit={handleSaveSettings}
      >
        <Card title={'API Configuration'}>
          <Flex className={'flex-col gap-3'}>
            <div>
              <Title level={5}>API Key</Title>
              <Text type="secondary">
                The {currentProvider.name} API uses API keys for authentication.
                Visit your API Keys page to retrieve the API key you'll use in
                your requests.
              </Text>
              <FormField
                name="api_key"
                aria-label="API key"
                className="mt-4"
                description={
                  /* The server no longer returns the API key in GET
                   * responses (06-llm-provider F-01 closure — secret
                   * was exposed to every user with read access).
                   * Field is write-only; leave empty to keep the
                   * existing value, or enter a new key to replace it.
                   */
                  'Leave empty to keep the current key. Type a new value to replace it.'
                }
              >
                <PasswordInput
                  placeholder={
                    'Insert API key (leave empty to keep current value)'
                  }
                  showLabel="Show API key"
                  hideLabel="Hide API key"
                />
              </FormField>
            </div>

            <div>
              <Title level={5}>Base URL</Title>
              <Text type="secondary">
                The base{' '}
                {currentProvider.provider_type === 'gemini'
                  ? 'OpenAI-compatible'
                  : ''}{' '}
                endpoint to use. See the {currentProvider.name} documentation{' '}
                for more information.
              </Text>
              <FormField name="base_url" aria-label="Base URL" className="mt-4">
                <Input placeholder={'Base URL'} />
              </FormField>
            </div>
          </Flex>

          {canEditProvider && (
            <>
              <Separator className="!my-3" />
              <Flex justify="end">
                <Button type="submit" disabled={!isDirty}>
                  Save
                </Button>
              </Flex>
            </>
          )}
        </Card>
      </Form>

      <LlmModelsSection />

      {/* User Groups Assignment */}
      <ProviderGroupAssignmentCard />

      {/* Proxy Settings - For non-Local providers */}
      <ProviderProxySettingsForm
        initialSettings={
          currentProvider.proxy_settings || ({} as ProxySettings)
        }
        onSave={handleProxySettingsSave}
      />

      {/* Model Management Drawers */}
      <AddRemoteLlmModelDrawer />
      <EditLlmModelDrawer />
    </Flex>
  )
}
