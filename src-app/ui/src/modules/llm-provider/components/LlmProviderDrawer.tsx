import { z } from 'zod'
import { Button, Form, FormField, Input, PasswordInput, Select, Switch, Text, message, useForm, zodResolver } from '@/components/ui'
import { useEffect, useState } from 'react'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type CreateLlmProviderRequest,
  type UpdateLlmProviderRequest,
} from '@/api-client/types'

const PROVIDER_TYPES = [
  { label: 'Local', value: 'local' },
  { label: 'OpenAI', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
  { label: 'Groq', value: 'groq' },
  { label: 'Google Gemini', value: 'gemini' },
  { label: 'Mistral AI', value: 'mistral' },
  { label: 'DeepSeek', value: 'deepseek' },
  { label: 'OpenRouter', value: 'openrouter' },
  { label: 'Hugging Face', value: 'huggingface' },
  { label: 'Custom', value: 'custom' },
]

const providerSchema = z.object({
  name: z.string().min(1, 'Please enter a provider name'),
  provider_type: z.string().min(1, 'Please select a provider type'),
  api_key: z.string().optional(),
  base_url: z.string().optional(),
  enabled: z.boolean().optional(),
})

type ProviderValues = z.infer<typeof providerSchema>

export function LlmProviderDrawer() {
  const [loading, setLoading] = useState(false)

  const { isOpen: open, editingProvider: provider } = Stores.LlmProviderDrawer
  const canCreate = usePermission(Permissions.LlmProvidersCreate)
  const canEdit = usePermission(Permissions.LlmProvidersEdit)
  const canSave = provider ? canEdit : canCreate

  const form = useForm<ProviderValues>({
    resolver: zodResolver(providerSchema),
    defaultValues: {
      name: '',
      provider_type: 'local',
      api_key: '',
      base_url: '',
      enabled: true,
    },
  })

  const providerType = form.watch('provider_type')

  // Update form when editing provider
  useEffect(() => {
    if (provider && open) {
      form.reset({
        name: provider.name,
        provider_type: provider.provider_type,
        api_key: provider.api_key,
        base_url: provider.base_url,
        enabled: provider.enabled,
      })
    } else if (!provider && open) {
      form.reset({
        name: '',
        provider_type: 'local',
        api_key: '',
        base_url: '',
        enabled: true,
      })
    }
  }, [provider, open, form])

  const handleClose = () => {
    form.reset()
    Stores.LlmProviderDrawer.closeLlmProviderDrawer()
  }

  const handleSubmit = async (values: ProviderValues) => {
    setLoading(true)

    try {
      if (provider) {
        // Update existing provider
        const updateData: UpdateLlmProviderRequest = {
          name: values.name,
          api_key: values.api_key,
          base_url: values.base_url,
          enabled: values.enabled ?? true,
        }
        await Stores.LlmProvider.updateLlmProvider(provider.id, updateData)
        message.success('Provider updated successfully')
      } else {
        // Add new provider
        const createData: CreateLlmProviderRequest = {
          name: values.name,
          provider_type: values.provider_type,
          api_key: values.api_key,
          base_url: values.base_url,
          enabled: values.enabled ?? true,
        }
        await Stores.LlmProvider.createLlmProvider(createData)
        message.success('Provider added successfully')
      }

      handleClose()
    } catch (error: any) {
      console.error('Failed to save provider:', error)
      message.error(error?.message || 'Failed to save provider')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Drawer
      title={provider ? `Edit Provider: ${provider.name}` : 'Add Provider'}
      open={open}
      onClose={handleClose}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={handleClose} disabled={loading} data-testid="llm-provider-cancel-btn">
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button type="submit" form="llm-provider-form" loading={loading} data-testid="llm-provider-submit-btn">
              {provider ? 'Save' : 'Add'}
            </Button>
          )}
        </div>
      }
      size={600}
      mask={{ closable: false }}
    >
      <Form
        name="llm-provider-form"
        form={form}
        layout="vertical"
        onSubmit={handleSubmit}
        disabled={!canSave}
        data-testid="llm-provider-form"
      >
        <FormField
          name="name"
          label="Provider Name"
          required
        >
          <Input placeholder="My Custom Provider" data-testid="llm-provider-name-input" />
        </FormField>

        <FormField
          name="enabled"
          label="Enable Provider"
          valuePropName="checked"
        >
          <Switch aria-label="Enable or disable this provider" data-testid="llm-provider-enabled-switch" />
        </FormField>

        <FormField
          name="provider_type"
          label="Provider Type"
          required
        >
          <Select
            options={PROVIDER_TYPES}
            disabled={!!provider}
            placeholder="Select provider type"
            data-testid="llm-provider-type-select"
          />
        </FormField>

        {providerType === 'local' ? (
          <div className="mb-4">
            <Text type="secondary" data-testid="llm-provider-local-note">
              Local providers don't require API keys. Configure your local
              inference server separately.
            </Text>
          </div>
        ) : (
          <>
            <FormField
              name="api_key"
              label="API Key"
              description="Optional — if not set, users can provide their own keys"
            >
              <PasswordInput
                placeholder="Enter your API key"
                showLabel="Show API key"
                hideLabel="Hide API key"
                data-testid="llm-provider-api-key-input"
              />
            </FormField>

            <FormField name="base_url" label="Base URL">
              <Input placeholder="https://api.provider.com/v1" data-testid="llm-provider-base-url-input" />
            </FormField>
          </>
        )}

      </Form>
    </Drawer>
  )
}
