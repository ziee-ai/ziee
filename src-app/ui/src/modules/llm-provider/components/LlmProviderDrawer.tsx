import { EyeInvisibleOutlined, EyeTwoTone } from '@ant-design/icons'
import { App, Button, Form, Input, Select, Switch, Typography } from 'antd'
import { useEffect, useState } from 'react'
import { Drawer } from '@/components/common/Drawer.tsx'
import {
  createLlmProvider,
  updateLlmProvider,
} from '../store'
import { Stores } from '@/core/stores'
import type {
  CreateLlmProviderRequest,
  UpdateLlmProviderRequest,
} from '@/api-client/types'

const { Text } = Typography

const PROVIDER_TYPES = [
  { label: 'Local', value: 'local' },
  { label: 'OpenAI', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
  { label: 'Groq', value: 'groq' },
  { label: 'Google Gemini', value: 'gemini' },
  { label: 'Mistral AI', value: 'mistral' },
  { label: 'DeepSeek', value: 'deepseek' },
  { label: 'Hugging Face', value: 'huggingface' },
  { label: 'Custom', value: 'custom' },
]

export function LlmProviderDrawer() {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const { isOpen: open, editingProvider: provider } = Stores.LlmProviderDrawer

  // Update form when editing provider
  useEffect(() => {
    if (provider && open) {
      form.setFieldsValue({
        name: provider.name,
        provider_type: provider.provider_type,
        api_key: provider.api_key,
        base_url: provider.base_url,
        enabled: provider.enabled,
      })
    } else if (!provider && open) {
      form.setFieldsValue({
        provider_type: 'local',
        enabled: true,
      })
    }
  }, [provider, open, form])

  const handleClose = () => {
    form.resetFields()
    Stores.LlmProviderDrawer.closeLlmProviderDrawer()
  }

  const handleSubmit = async (values: any) => {
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
        await updateLlmProvider(provider.id, updateData)
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
        await createLlmProvider(createData)
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

  const requiresApiKey = (type: string) => {
    return type !== 'local' && type !== 'custom'
  }

  return (
    <Drawer
      title={provider ? `Edit Provider: ${provider.name}` : 'Add Provider'}
      open={open}
      onClose={handleClose}
      footer={null}
      width={600}
      maskClosable={false}
    >
      <Form
        name="llm-provider-form"
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
      >
        <Form.Item
          name="name"
          label="Provider Name"
          rules={[{ required: true, message: 'Please enter a provider name' }]}
        >
          <Input placeholder="My Custom Provider" />
        </Form.Item>

        <Form.Item
          name="provider_type"
          label="Provider Type"
          rules={[{ required: true, message: 'Please select a provider type' }]}
        >
          <Select
            options={PROVIDER_TYPES}
            disabled={!!provider}
            placeholder="Select provider type"
          />
        </Form.Item>

        <Form.Item dependencies={['provider_type']} noStyle>
          {({ getFieldValue }) => {
            const type = getFieldValue('provider_type')

            if (type === 'local') {
              return (
                <div className="mb-4">
                  <Text type="secondary">
                    Local providers don't require API keys. Configure your local
                    inference server separately.
                  </Text>
                </div>
              )
            }

            return (
              <>
                <Form.Item
                  name="api_key"
                  label="API Key"
                  rules={
                    requiresApiKey(type)
                      ? [{ required: true, message: 'API key is required' }]
                      : []
                  }
                >
                  <Input.Password
                    placeholder="Enter your API key"
                    iconRender={visible =>
                      visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                    }
                  />
                </Form.Item>

                <Form.Item name="base_url" label="Base URL">
                  <Input placeholder="https://api.provider.com/v1" />
                </Form.Item>
              </>
            )
          }}
        </Form.Item>

        <Form.Item
          name="enabled"
          label="Enable Provider"
          valuePropName="checked"
        >
          <Switch aria-label="Enable or disable this provider" />
        </Form.Item>

        <div className="flex justify-end gap-3 pt-4">
          <Button onClick={handleClose} disabled={loading}>
            Cancel
          </Button>
          <Button type="primary" htmlType="submit" loading={loading}>
            {provider ? 'Update' : 'Add'} Provider
          </Button>
        </div>
      </Form>
    </Drawer>
  )
}
