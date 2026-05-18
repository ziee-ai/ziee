import { useState, useEffect } from 'react'
import { useApiKeysStepStore } from './ApiKeysStep.store'
import {
  Typography,
  Form,
  Input,
  Spin,
  Alert,
  Tag,
  Flex,
  Menu,
} from 'antd'
import { ApiOutlined, CheckCircleOutlined } from '@ant-design/icons'
import type { OnboardingStepProps } from '@/modules/onboarding-screen/types/onboarding'
import { Stores } from '@/core/stores'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'

const { Title, Text, Paragraph } = Typography

export default function ApiKeysStep({ registerBeforeNext }: OnboardingStepProps) {
  const enteredApiKeys = Stores.ApiKeysStep.enteredApiKeys
  const providers = Stores.ApiKeysStep.providers
  const userKeys = Stores.ApiKeysStep.userKeys
  const loading = Stores.ApiKeysStep.loadingProviders
  const error = Stores.ApiKeysStep.providersError

  const [selectedId, setSelectedId] = useState<string | null>(null)

  // Set initial selected provider once providers load
  useEffect(() => {
    if (providers.length > 0 && !selectedId) {
      setSelectedId(providers[0].id)
    }
  }, [providers])

  useEffect(() => {
    Stores.OnboardingScreen.setReady(true)
    registerBeforeNext(async () => {
      const { enteredApiKeys, saveKey } = useApiKeysStepStore.getState()
      const keysToSave = Object.entries(enteredApiKeys).filter(([, v]) => v.trim())
      for (const [providerId, key] of keysToSave) {
        await saveKey(providerId, key.trim())
      }
    })
  }, [])

  if (loading) {
    return (
      <div className="flex justify-center mt-8">
        <Spin />
      </div>
    )
  }

  if (providers.length === 0) {
    return (
      <div className="max-w-lg">
        <div className="flex items-center gap-3 mb-4">
          <ApiOutlined className="text-3xl text-blue-500" />
          <Title level={3} className="!mb-0">
            AI Providers
          </Title>
        </div>
        <Paragraph type="secondary">
          No AI providers are currently enabled. An administrator can add
          providers in the Admin settings.
        </Paragraph>
      </div>
    )
  }

  const currentProvider = providers.find(p => p.id === selectedId) ?? providers[0]
  const hasUserKey = !!userKeys[currentProvider.id]

  const menuItems = providers.map(provider => {
    const IconComponent = PROVIDER_ICONS[provider.provider_type] || PROVIDER_ICONS.custom
    return {
      key: provider.id,
      label: (
        <Flex className="flex-row gap-2 items-center h-full">
          <IconComponent className="text-lg" />
          <div className="flex-1 flex items-center h-full overflow-x-hidden">
            <Typography.Text ellipsis>{provider.name}</Typography.Text>
          </div>
        </Flex>
      ),
    }
  })

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-3 mb-3">
        <ApiOutlined className="text-2xl text-blue-500" />
        <Title level={4} className="!mb-0">
          AI Providers
        </Title>
      </div>

      <Paragraph type="secondary" className="mb-3">
        You can add your own API keys for any provider. Keys set by your
        administrator are already working — entering your own will override
        them.
      </Paragraph>

      {error && (
        <Alert type="error" message={error} showIcon className="mb-3" />
      )}

      {/* Two-column layout */}
      <div className="flex flex-1 mb-4">
        {/* Left sidebar */}
        <div className="w-40 flex-shrink-0 pt-1">
          <Menu
            className={`
              w-full h-full !m-0
              [&_.ant-menu]:!px-0
              [&_.ant-menu-item]:!h-8
              [&_.ant-menu-item]:!leading-[32px]
              !bg-transparent !border-none`}
            selectedKeys={[currentProvider.id]}
            items={menuItems}
            onClick={({ key }) => setSelectedId(key)}
          />
        </div>

        {/* Right content */}
        <div className="flex-1 px-4">
          <Flex align="center" gap={8} className="mb-1">
            {(() => {
              const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom
              return <IconComponent className="text-xl" />
            })()}
            <Text strong className="text-base">{currentProvider.name}</Text>
            {(currentProvider.api_key_configured || hasUserKey) && (
              <Tag icon={<CheckCircleOutlined />} color="success">
                {hasUserKey ? 'Your key configured' : 'Admin key configured'}
              </Tag>
            )}
          </Flex>

          <Text type="secondary" className="block text-xs mb-4">
            {currentProvider.api_key_configured
              ? 'Enter your own key to override the admin key.'
              : 'No system key is set. Enter your own to use this provider.'}
          </Text>

          <Form layout="vertical" className="max-w-sm">
            <Form.Item label="Your API Key" className="!mb-2">
              <Input.Password
                value={enteredApiKeys[currentProvider.id] || ''}
                onChange={e =>
                  Stores.ApiKeysStep.setApiKey(currentProvider.id, e.target.value)
                }
                placeholder={
                  hasUserKey
                    ? 'Enter to update your key'
                    : currentProvider.api_key_configured
                      ? 'Enter to override admin key'
                      : 'sk-...'
                }
              />
            </Form.Item>
          </Form>
        </div>
      </div>
    </div>
  )
}
