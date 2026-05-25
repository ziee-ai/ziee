import { EyeInvisibleOutlined, EyeTwoTone } from '@ant-design/icons'
import { App, Button, Card, Flex, Form, Input, Typography } from 'antd'
import { useEffect, useState } from 'react'
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

const { Title, Text } = Typography

export function RemoteProviderSettings() {
  const { message } = App.useApp()
  const { providerId } = useParams<{ providerId?: string }>()

  const [form] = Form.useForm()
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false)
  const [pendingSettings, setPendingSettings] = useState<any>(null)

  // Store data
  const { error } = Stores.LlmProvider
  const canEditProvider = usePermission(Permissions.LlmProvidersEdit)

  // Get current provider and its models
  const currentProvider = Stores.LlmProvider.providers.find(
    p => p.id === providerId,
  )


  const handleFormChange = (changedValues: any) => {
    if (!currentProvider) return

    setHasUnsavedChanges(true)
    setPendingSettings((prev: any) => ({ ...prev, ...changedValues }))
  }

  const handleSaveSettings = async () => {
    if (!currentProvider || !pendingSettings) return

    try {
      await Stores.LlmProvider.updateLlmProvider(
        currentProvider.id,
        pendingSettings,
      )

      setHasUnsavedChanges(false)
      setPendingSettings(null)
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
  }, [error, message])

  // Update forms when provider changes
  useEffect(() => {
    if (currentProvider) {
      form.setFieldsValue({
        api_key: currentProvider.api_key,
        base_url: currentProvider.base_url,
      })
      // Clear unsaved changes when switching providers
      setHasUnsavedChanges(false)
      setPendingSettings(null)
    }
  }, [currentProvider, form])

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
        initialValues={{
          api_key: currentProvider.api_key,
          base_url: currentProvider.base_url,
        }}
        onValuesChange={handleFormChange}
      >
        <Card
          title={'API Configuration'}
          extra={
            canEditProvider && (
              <Button
                type="primary"
                onClick={handleSaveSettings}
                disabled={!hasUnsavedChanges}
              >
                Save
              </Button>
            )
          }
        >
          <Flex className={'flex-col gap-3'}>
            <div>
              <Title level={5}>API Key</Title>
              <Text type="secondary">
                The {currentProvider.name} API uses API keys for authentication.
                Visit your API Keys page to retrieve the API key you'll use in
                your requests.
              </Text>
              <Form.Item
                name="api_key"
                style={{ marginBottom: 0, marginTop: 16 }}
                help={
                  /* The server no longer returns the API key in GET
                   * responses (06-llm-provider F-01 closure — secret
                   * was exposed to every user with read access).
                   * Field is write-only; leave empty to keep the
                   * existing value, or enter a new key to replace it.
                   */
                  'Leave empty to keep the current key. Type a new value to replace it.'
                }
              >
                <Input.Password
                  placeholder={
                    'Insert API key (leave empty to keep current value)'
                  }
                  iconRender={visible =>
                    visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                  }
                />
              </Form.Item>
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
              <Form.Item
                name="base_url"
                style={{ marginBottom: 0, marginTop: 16 }}
              >
                <Input placeholder={'Base URL'} />
              </Form.Item>
            </div>
          </Flex>
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
