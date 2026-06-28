import {
  Button,
  Confirm,
  Flex,
  Form,
  FormField,
  Input,
  Switch,
  Title,
  Tooltip,
  message,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import {
  Check,
  X,
  Trash2,
  Pencil,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'
import { Permissions, type LlmProvider } from '@/api-client/types'

const nameSchema = z.object({
  name: z.string().min(1, 'Name is required'),
})
type NameValues = z.infer<typeof nameSchema>

export function ProviderHeader() {
  const [isEditingName, setIsEditingName] = useState(false)
  const form = useForm<NameValues>({
    resolver: zodResolver(nameSchema),
    defaultValues: { name: '' },
  })
  const navigate = useNavigate()
  const { providerId } = useParams<{ providerId?: string }>()

  const canEdit = usePermission(Permissions.LlmProvidersEdit)
  const canDelete = usePermission(Permissions.LlmProvidersDelete)

  // Get current provider from store
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === providerId,
  )

  useEffect(() => {
    setIsEditingName(false)
  }, [currentProvider?.id])

  useEffect(() => {
    if (isEditingName && currentProvider) {
      form.reset({ name: currentProvider.name })
    }
  }, [isEditingName, currentProvider, form])

  // Helper functions for provider validation
  const canEnableProvider = (provider: LlmProvider): boolean => {
    if (provider.enabled) return true // Already enabled
    if (provider.provider_type === 'local') return true
    return Stores.LlmProvider.llmProviderHasCredentials(provider)
  }

  const getEnableDisabledReason = (provider: LlmProvider): string | null => {
    if (provider.enabled) return null
    if (provider.provider_type === 'local') return null
    if (!Stores.LlmProvider.llmProviderHasCredentials(provider)) {
      return 'API key is required for remote providers'
    }
    return null
  }

  const handleProviderToggle = async (providerId: string, enabled: boolean) => {
    if (!currentProvider) return

    try {
      await Stores.LlmProvider.updateLlmProvider(providerId, {
        enabled: enabled,
      })
      message.success(
        `${currentProvider?.name || 'Provider'} ${enabled ? 'enabled' : 'disabled'}`,
      )
    } catch (error: any) {
      console.error('Failed to update provider:', error)
      message.error(error?.message || 'Failed to update provider')
    }
  }

  const handleDeleteProvider = async () => {
    if (!currentProvider) return
    try {
      await Stores.LlmProvider.deleteLlmProvider(currentProvider.id)
      navigate(`/settings/llm-providers`, { replace: true })
      message.success('Provider deleted successfully')
    } catch (error: any) {
      console.error('Failed to delete provider:', error)
      message.error(error?.message || 'Failed to delete provider')
    }
  }

  const handleSaveName = async (values: NameValues) => {
    if (!currentProvider) return
    await Stores.LlmProvider.updateLlmProvider(currentProvider.id, {
      name: values.name,
    })
    setIsEditingName(false)
  }

  // Return early if no provider
  if (!currentProvider) {
    return null
  }

  const IconComponent =
    PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom

  return (
    <Flex justify="between" align="center">
      <Flex align="center" gap="middle">
        <IconComponent className="text-2xl" />
        <Form
          name="provider-header-name-form"
          className={isEditingName ? 'block' : 'hidden'}
          form={form}
          layout="inline"
          onSubmit={handleSaveName}
        >
          <div className={'flex items-center gap-2 w-full flex-wrap'}>
            <FormField name="name" aria-label="Provider name">
              <Input className={'!text-lg'} />
            </FormField>
            <div className={'flex items-center gap-2'}>
              <Button type="submit" aria-label="Save provider name">
                <Check aria-hidden="true" />
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={() => setIsEditingName(false)}
                aria-label="Cancel editing provider name"
              >
                <X aria-hidden="true" />
              </Button>
            </div>
          </div>
        </Form>
        <div
          className={
            'flex items-center gap-2 overflow-x-hidden w-full ' +
            (isEditingName ? 'hidden' : 'flex')
          }
        >
          <Title
            level={4}
            className={'!m-0 flex-1 overflow-x-hidden'}
          >
            {currentProvider.name}
          </Title>
          <div className={'flex items-center'}>
            {canEdit && (
              <Button
                variant="ghost"
                onClick={() => {
                  setIsEditingName(!isEditingName)
                }}
                aria-label="Edit provider name"
              >
                <Pencil aria-hidden="true" />
              </Button>
            )}
            {canDelete && !currentProvider.built_in && (
              <Confirm
                title="Delete Provider"
                description={`Are you sure you want to delete "${currentProvider.name}"? This action cannot be undone.`}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
                onConfirm={handleDeleteProvider}
              >
                <Button
                  variant="ghost"
                  aria-label="Delete provider"
                >
                  <Trash2 aria-hidden="true" />
                </Button>
              </Confirm>
            )}
          </div>
        </div>
      </Flex>
      {canEdit && (() => {
        const disabledReason = getEnableDisabledReason(currentProvider)
        const switchElement = (
          <Switch
            checked={currentProvider.enabled}
            disabled={
              !currentProvider.enabled && !canEnableProvider(currentProvider)
            }
            onChange={enabled =>
              handleProviderToggle(currentProvider.id, enabled)
            }
            aria-label={`${currentProvider.enabled ? 'Disable' : 'Enable'} ${currentProvider.name} provider`}
          />
        )

        if (disabledReason && !currentProvider.enabled) {
          return <Tooltip title={disabledReason}>{switchElement}</Tooltip>
        }
        return switchElement
      })()}
    </Flex>
  )
}
