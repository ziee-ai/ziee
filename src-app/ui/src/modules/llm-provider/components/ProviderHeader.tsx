import {
  Button,
  Confirm,
  Flex,
  Input,
  Switch,
  Title,
  Tooltip,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
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
  // API keys are no longer required to enable a provider (users supply their
  // own via profile settings), so there is no precondition that blocks
  // enabling — keep these helpers for clarity but drop the dead/misleading
  // "API key required" branch that could never trigger.
  const canEnableProvider = (_provider: LlmProvider): boolean => true

  const getEnableDisabledReason = (_provider: LlmProvider): string | null =>
    null

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
        {/* Plain horizontal <form> (NOT the kit <Form>/<FormField>): the kit Form's
            FieldGroup/responsive Field collapsed the input to ~20px and stacked the
            buttons below. A flat flex row with the Input registered directly keeps a
            growing input + trailing confirm/cancel on one line. */}
        <form
          name="provider-header-name-form"
          className={
            (isEditingName ? 'flex' : 'hidden') + ' items-center gap-2 w-full'
          }
          onSubmit={form.handleSubmit(handleSaveName)}
          data-testid="llm-provider-header-name-form"
        >
          <Input
            {...form.register('name')}
            aria-label="Provider name"
            className={'flex-1 min-w-0 !text-lg'}
            data-testid="llm-provider-header-name-input"
          />
          <Button type="submit" size="icon" icon={<Check aria-hidden="true" />} tooltip="Save provider name" aria-label="Save provider name" data-testid="llm-provider-header-save-name-btn" />
          <Button
            type="button"
            size="icon"
            variant="ghost"
            icon={<X aria-hidden="true" />}
            onClick={() => setIsEditingName(false)}
            tooltip="Cancel editing provider name"
            aria-label="Cancel editing provider name"
            data-testid="llm-provider-header-cancel-name-btn"
          />
        </form>
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
                size="icon"
                icon={<Pencil aria-hidden="true" />}
                onClick={() => {
                  setIsEditingName(!isEditingName)
                }}
                tooltip="Edit provider name"
                aria-label="Edit provider name"
                data-testid="llm-provider-header-edit-name-btn"
              />
            )}
            {canDelete && !currentProvider.built_in && (
              <Confirm
                title="Delete Provider"
                description={`Are you sure you want to delete "${currentProvider.name}"? This action cannot be undone.`}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
                onConfirm={handleDeleteProvider}
                data-testid="llm-provider-delete-confirm"
              >
                <Button
                  variant="ghost"
                  size="icon"
                  icon={<Trash2 aria-hidden="true" />}
                  tooltip="Delete provider"
                  aria-label="Delete provider"
                  data-testid="llm-provider-delete-btn"
                />
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
            tooltip={`${currentProvider.enabled ? 'Disable' : 'Enable'} ${currentProvider.name} provider`}
            data-testid="llm-provider-header-enabled-switch"
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
