import {
  App,
  Button,
  Flex,
  Form,
  Input,
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
import {
  deleteLlmProvider,
  updateLlmProvider,
  Stores,
  llmProviderHasCredentials,
} from '../store'
import { PROVIDER_ICONS } from '../constants'
import type { LlmProvider } from '@/api-client/types'

export function ProviderHeader() {
  const [isEditingName, setIsEditingName] = useState(false)
  const [form] = Form.useForm()
  const navigate = useNavigate()
  const { message, modal } = App.useApp()
  const { providerId } = useParams<{ providerId?: string }>()

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
  const canEnableProvider = (provider: LlmProvider): boolean => {
    if (provider.enabled) return true // Already enabled
    if (provider.provider_type === 'local') return true
    return llmProviderHasCredentials(provider)
  }

  const getEnableDisabledReason = (provider: LlmProvider): string | null => {
    if (provider.enabled) return null
    if (provider.provider_type === 'local') return null
    if (!llmProviderHasCredentials(provider)) {
      return 'API key is required for remote providers'
    }
    return null
  }

  const handleProviderToggle = async (providerId: string, enabled: boolean) => {
    if (!currentProvider) return

    try {
      await updateLlmProvider(providerId, {
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

    // Don't allow deleting built-in providers
    if (currentProvider.built_in) {
      message.warning('Built-in providers cannot be deleted')
      return
    }

    modal.confirm({
      title: 'Confirm Deletion',
      content: `Are you sure you want to delete "${currentProvider.name}"? This action cannot be undone.`,
      okText: 'Delete',
      okType: 'danger',
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          await deleteLlmProvider(currentProvider.id)
          navigate(`/settings/llm-providers`, {
            replace: true,
          })
          message.success('Provider deleted successfully')
        } catch (error: any) {
          console.error('Failed to delete provider:', error)
          message.error(error?.message || 'Failed to delete provider')
        }
      },
    })
  }

  // Return early if no provider
  if (!currentProvider) {
    return null
  }

  const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom

  return (
    <Flex justify="space-between" align="center">
      <Flex align="center" gap="middle">
        <IconComponent className="text-2xl" />
        <Form
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
                    await updateLlmProvider(currentProvider.id, {
                      name: values.name,
                    })
                    setIsEditingName(false)
                  })
                }}
              >
                <CheckOutlined />
              </Button>
              <Button onClick={() => setIsEditingName(false)}>
                <CloseOutlined />
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
            <Button
              type={'text'}
              onClick={() => {
                setIsEditingName(!isEditingName)
              }}
            >
              <EditOutlined />
            </Button>
            {!currentProvider.built_in && (
              <Button type={'text'} danger onClick={handleDeleteProvider}>
                <DeleteOutlined />
              </Button>
            )}
          </div>
        </div>
      </Flex>
      {(() => {
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
