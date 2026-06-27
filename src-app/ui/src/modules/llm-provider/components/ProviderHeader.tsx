import {
  App,
  Button,
  Flex,
  Form,
  Input,
  Popconfirm,
  Switch,
  Tooltip,
  Typography,
} from 'antd'
import {
  CheckOutlined,
  CloseOutlined,
  DeleteOutlined,
  EditOutlined,
} from '@ant-design/icons'
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'
import { Permissions, type LlmProvider } from '@/api-client/types'

export function ProviderHeader() {
  const [isEditingName, setIsEditingName] = useState(false)
  const [form] = Form.useForm()
  const navigate = useNavigate()
  const { message } = App.useApp()
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
      form.setFieldsValue({ name: currentProvider.name })
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

  // Return early if no provider
  if (!currentProvider) {
    return null
  }

  const IconComponent =
    PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom

  return (
    <Flex justify="space-between" align="center">
      <Flex align="center" gap="middle">
        <IconComponent className="text-2xl" />
        <Form
          name="provider-header-name-form"
          style={{
            display: isEditingName ? 'block' : 'none',
          }}
          form={form}
          layout="inline"
          initialValues={{ name: currentProvider.name }}
        >
          <div className={'flex items-center gap-2 w-full flex-wrap'}>
            <Form.Item
              name="name"
              style={{ margin: 0 }}
              rules={[{ required: true, message: 'Name is required' }]}
            >
              <Input className={'!text-lg'} />
            </Form.Item>
            <div className={'flex items-center gap-2'}>
              <Button
                type={'primary'}
                onClick={() => {
                  form.validateFields().then(async values => {
                    await Stores.LlmProvider.updateLlmProvider(
                      currentProvider.id,
                      {
                        name: values.name,
                      },
                    )
                    setIsEditingName(false)
                  })
                }}
                aria-label="Save provider name"
              >
                <CheckOutlined aria-hidden="true" />
              </Button>
              <Button
                onClick={() => setIsEditingName(false)}
                aria-label="Cancel editing provider name"
              >
                <CloseOutlined aria-hidden="true" />
              </Button>
            </div>
          </div>
        </Form>
        <div
          className={'flex items-center gap-2 overflow-x-hidden w-full'}
          style={{
            display: isEditingName ? 'none' : 'flex',
          }}
        >
          <Typography.Title
            level={4}
            ellipsis
            className={'!m-0 flex-1 overflow-x-hidden'}
          >
            {currentProvider.name}
          </Typography.Title>
          <div className={'flex items-center'}>
            {canEdit && (
              <Button
                type={'text'}
                onClick={() => {
                  setIsEditingName(!isEditingName)
                }}
                aria-label="Edit provider name"
              >
                <EditOutlined aria-hidden="true" />
              </Button>
            )}
            {canDelete && !currentProvider.built_in && (
              <Popconfirm
                title="Delete Provider"
                description={`Are you sure you want to delete "${currentProvider.name}"? This action cannot be undone.`}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
                onConfirm={handleDeleteProvider}
              >
                <Button
                  type={'text'}
                  danger
                  aria-label="Delete provider"
                >
                  <DeleteOutlined aria-hidden="true" />
                </Button>
              </Popconfirm>
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
