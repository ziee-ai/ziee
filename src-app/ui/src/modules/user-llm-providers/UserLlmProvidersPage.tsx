import { useState, useEffect } from 'react'
import {
  Typography,
  Form,
  Input,
  Button,
  Space,
  Spin,
  Alert,
  Tag,
  Flex,
  Menu,
  Dropdown,
  Empty,
} from 'antd'
import { CheckCircleOutlined } from '@ant-design/icons'
import { IoIosArrowDown } from 'react-icons/io'
import { Stores } from '@/core/stores'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

const { Title, Text } = Typography

// Displayed in the input when a key is already saved — long enough to look like a real key
const KEY_DISPLAY_PLACEHOLDER = '••••••••••••••••••••••••'

export default function UserLlmProvidersPage() {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [keyValue, setKeyValue] = useState('')
  const [savingFor, setSavingFor] = useState<string | null>(null)

  const providers = Stores.UserLlmProviders.providers
  const userKeys = Stores.UserLlmProviders.userKeys
  const loading = Stores.UserLlmProviders.loading
  const error = Stores.UserLlmProviders.error
  const saving = Stores.UserLlmProviders.saving
  const windowMinSize = useWindowMinSize()

  // Set initial selected provider once providers load
  useEffect(() => {
    if (providers.length > 0 && !selectedId) {
      setSelectedId(providers[0].id)
    }
  }, [providers])

  // Populate input with display placeholder when provider has a key
  useEffect(() => {
    const hasKey = selectedId ? !!userKeys[selectedId] : false
    setKeyValue(hasKey ? KEY_DISPLAY_PLACEHOLDER : '')
  }, [selectedId, userKeys])

  const currentProvider = providers.find(p => p.id === selectedId) ?? null
  const hasUserKey = !!currentProvider && !!userKeys[currentProvider.id]

  const handleSave = async () => {
    if (!selectedId) return
    const key = keyValue.trim()
    if (!key || key === KEY_DISPLAY_PLACEHOLDER) return

    setSavingFor(selectedId)
    try {
      await Stores.UserLlmProviders.saveKey(selectedId, key)
    } finally {
      setSavingFor(null)
    }
  }

  const handleDelete = async () => {
    if (!selectedId) return
    await Stores.UserLlmProviders.deleteKey(selectedId)
  }

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

  const ProviderMenu = () => (
    <Menu
      className={`
        w-full h-full !m-0
        [&_.ant-menu]:!px-0
        [&_.ant-menu-item]:!h-8
        [&_.ant-menu-item]:!leading-[32px]
        !bg-transparent !border-none`}
      selectedKeys={selectedId ? [selectedId] : []}
      items={menuItems}
      onClick={({ key }) => setSelectedId(key)}
    />
  )

  if (loading) {
    return (
      <div className="flex items-center justify-center h-32">
        <Spin />
      </div>
    )
  }

  if (error) {
    return <Alert type="error" message={error} showIcon className="m-6" />
  }

  const renderContent = () => {
    if (providers.length === 0) {
      return (
        <Empty
          description="No LLM providers are available."
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      )
    }

    if (!currentProvider) {
      return (
        <Empty
          description="No provider selected"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      )
    }

    const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom

    return (
      <div className="max-w-lg">
        <Flex align="center" gap={10} className="mb-1">
          <IconComponent className="text-2xl" />
          <Title level={4} className="!mb-0">
            {currentProvider.name}
          </Title>
          <Tag color={hasUserKey ? 'green' : currentProvider.api_key_configured ? 'blue' : 'orange'}>
            {hasUserKey ? (
              <><CheckCircleOutlined /> Your key configured</>
            ) : currentProvider.api_key_configured ? (
              <><CheckCircleOutlined /> Admin key configured</>
            ) : (
              'No admin key'
            )}
          </Tag>
        </Flex>

        <Text type="secondary" className="block mb-6">
          Your personal key takes priority over the system key when making requests.
        </Text>

        <Form layout="vertical">
          <Form.Item label="Your API Key">
            <Input.Password
              value={keyValue}
              onChange={e => setKeyValue(e.target.value)}
              onFocus={() => {
                if (keyValue === KEY_DISPLAY_PLACEHOLDER) setKeyValue('')
              }}
              placeholder={hasUserKey ? 'Enter new key to replace' : 'Enter your API key (e.g. sk-...)'}
            />
          </Form.Item>
          <Space>
            <Button
              type="primary"
              onClick={handleSave}
              loading={savingFor === currentProvider.id || saving}
              disabled={!keyValue.trim() || keyValue === KEY_DISPLAY_PLACEHOLDER}
            >
              {hasUserKey ? 'Update Key' : 'Save Key'}
            </Button>
            {hasUserKey && (
              <Button
                danger
                onClick={handleDelete}
                loading={savingFor === currentProvider.id}
              >
                Remove Key
              </Button>
            )}
          </Space>
        </Form>
      </div>
    )
  }

  return (
    <div className="flex w-full h-full">
      {/* Desktop sidebar */}
      {!windowMinSize.sm && (
        <div className="w-42 flex flex-col gap-2 h-full pt-3">
          <div className="w-full px-3">
            <Title level={4} className="!m-0 !leading-tight">
              Providers
            </Title>
          </div>
          <div className="flex-1 pl-2 overflow-y-auto">
            <ProviderMenu />
          </div>
        </div>
      )}

      {/* Main content */}
      <div className="flex flex-1">
        <div className="flex w-full flex-col py-3 px-3 overflow-y-auto">
          <div className="flex flex-col flex-1 max-w-3xl self-center w-full">
            {/* Mobile dropdown */}
            {windowMinSize.sm && providers.length > 0 && (
              <div className="w-full flex flex-row gap-2 items-center mb-4">
                <Dropdown
                  className="w-full"
                  menu={{
                    items: menuItems,
                    onClick: ({ key }) => setSelectedId(key),
                    selectedKeys: selectedId ? [selectedId] : [],
                  }}
                  trigger={['click']}
                >
                  <Button className="w-fit" size="large">
                    {currentProvider ? (
                      <Flex className="gap-2 items-center">
                        {(() => {
                          const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom
                          return <IconComponent className="text-lg" />
                        })()}
                        {currentProvider.name}
                      </Flex>
                    ) : (
                      'Select Provider'
                    )}
                    <IoIosArrowDown />
                  </Button>
                </Dropdown>
              </div>
            )}
            {renderContent()}
          </div>
        </div>
      </div>
    </div>
  )
}
