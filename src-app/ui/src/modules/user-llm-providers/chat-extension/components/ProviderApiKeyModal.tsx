import { z } from 'zod'
import {
  Dialog,
  Form,
  FormField,
  useForm,
  zodResolver,
  PasswordInput,
  Alert,
  Button,
  Paragraph,
  Text,
  Link,
} from '@/components/ui'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'

interface ProviderApiKeyModalProps {
  providerId: string
  providerName: string
  modelId: string
  onSuccess: (modelId: string) => void
  onCancel: () => void
}

const schema = z.object({
  apiKey: z.string().min(1, 'API key cannot be empty'),
})
type FormValues = z.infer<typeof schema>

/**
 * ProviderApiKeyModal
 * Shown inline inside ModelSelector when user selects a model whose
 * provider has no API key configured. Lets the user save their own key
 * before the model is selected.
 */
export function ProviderApiKeyModal({
  providerId,
  providerName,
  modelId,
  onSuccess,
  onCancel,
}: ProviderApiKeyModalProps) {
  const navigate = useNavigate()
  const { saving } = Stores.UserProviderKeys

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { apiKey: '' },
  })

  const onValidSubmit = async ({ apiKey }: { apiKey: string }) => {
    try {
      await Stores.UserProviderKeys.saveKey(providerId, apiKey.trim())
      onSuccess(modelId)
    } catch (err: any) {
      form.setError('root', { message: err.message || 'Failed to save API key' })
    }
  }
  const handleOk = form.handleSubmit(onValidSubmit)

  const rootError = form.formState.errors.root?.message

  return (
    <Dialog
      open
      data-testid="ullm-apikey-dialog"
      onOpenChange={v => { if (!v) onCancel() }}
      title={`API Key Required — ${providerName}`}
      footer={
        <>
          <Button variant="outline" data-testid="ullm-apikey-cancel-button" onClick={onCancel}>
            Cancel
          </Button>
          <Button data-testid="ullm-apikey-save-button" onClick={handleOk} loading={saving}>
            Save &amp; Select Model
          </Button>
        </>
      }
    >
      <Paragraph type="secondary">
        This provider doesn&apos;t have a system API key configured. Enter your
        own API key to use models from <strong>{providerName}</strong>.
      </Paragraph>
      <Form form={form} data-testid="ullm-apikey-form" onSubmit={onValidSubmit} layout="vertical">
        <FormField name="apiKey" label="API Key">
          <PasswordInput
            data-testid="ullm-apikey-password-input"
            placeholder="sk-..."
            autoFocus
            showLabel="Show API key"
            hideLabel="Hide API key"
            onKeyDown={e => { if (e.key === 'Enter') handleOk() }}
          />
        </FormField>
        {rootError && <Alert tone="error" data-testid="ullm-apikey-error-alert" title={rootError} />}
      </Form>
      <Text type="secondary" className="text-xs">
        Your key is stored securely and only used for inference. You can manage
        keys in{' '}
        <Link
          href="/settings/user-llm-providers"
          onClick={e => {
            // Client-side navigation (react-router) instead of a full-page reload;
            // close the modal first so it isn't left open over the settings page.
            e.preventDefault()
            onCancel()
            navigate('/settings/user-llm-providers')
          }}
        >
          Settings → LLM Providers
        </Link>
        .
      </Text>
    </Dialog>
  )
}
