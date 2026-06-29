import { useState, useEffect } from 'react'
import {
  Button,
  Space,
  Spin,
  Alert,
  Tag,
  Flex,
  Dropdown,
  Empty,
  Title,
  Text,
  Form,
  FormField,
  useForm,
  PasswordInput,
  Menu,
} from '@/components/ui'
import { CircleCheck } from 'lucide-react'
import { IoIosArrowDown } from 'react-icons/io'
import { Stores } from '@/core/stores'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

const KEY_DISPLAY_PLACEHOLDER = '••••••••••••••••••••••••'

export default function UserLlmProvidersPage() {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [savingFor, setSavingFor] = useState<string | null>(null)

  const providers = Stores.UserLlmProviders.providers
  const userKeys = Stores.UserLlmProviders.userKeys
  const loading = Stores.UserLlmProviders.loading
  const error = Stores.UserLlmProviders.error
  const saving = Stores.UserLlmProviders.saving
  const windowMinSize = useWindowMinSize()

  const form = useForm<{ apiKey: string }>({ defaultValues: { apiKey: '' } })
  // Shadow the old state variable so downstream logic is unchanged
  const keyValue = form.watch('apiKey') ?? ''
  const setKeyValue = (v: string) => form.setValue('apiKey', v)

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
            <Text ellipsis>{provider.name}</Text>
          </div>
        </Flex>
      ),
    }
  })

  const ProviderMenu = () => (
    <Menu
      data-testid="ullm-provider-menu"
      className="w-full h-full"
      selectedKey={selectedId ?? undefined}
      items={menuItems}
      onSelect={(key) => setSelectedId(key)}
      mode="vertical"
      aria-label="Providers"
    />
  )

  if (loading) {
    return (
      <div className="flex items-center justify-center h-32">
        <Spin label="Loading" />
      </div>
    )
  }

  if (error) {
    return <Alert tone="error" data-testid="ullm-error-alert" title={error} className="m-6" />
  }

  const renderContent = () => {
    if (providers.length === 0) {
      return (
        <Empty
          data-testid="ullm-no-providers-empty"
          description={
            <span>
              No AI providers are available yet.
              <br />
              An administrator needs to add a provider and a model before you
              can configure keys here.
            </span>
          }
        />
      )
    }

    if (!currentProvider) {
      return (
        <Empty
          data-testid="ullm-no-selection-empty"
          description="No provider selected"
        />
      )
    }

    const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom

    return (
      <div className="max-w-lg">
        <Flex align="center" gap="small" className="mb-1">
          <IconComponent className="text-2xl" />
          <Title level={4} className="!mb-0">
            {currentProvider.name}
          </Title>
          <Tag data-testid="ullm-key-status-tag" tone={hasUserKey ? 'success' : currentProvider.api_key_configured ? 'info' : 'warning'}>
            {hasUserKey ? (
              <><CircleCheck /> Your key configured</>
            ) : currentProvider.api_key_configured ? (
              <><CircleCheck /> Admin key configured</>
            ) : (
              'No admin key'
            )}
          </Tag>
        </Flex>

        <Text type="secondary" className="block mb-6">
          Your personal key takes priority over the system key when making requests.
        </Text>

        <Form form={form} data-testid="ullm-key-form" onSubmit={handleSave} layout="vertical">
          <FormField name="apiKey" label="Your API Key">
            <PasswordInput data-testid="ullm-key-password-input" showLabel="Show" hideLabel="Hide"
              onFocus={() => {
                if (keyValue === KEY_DISPLAY_PLACEHOLDER) setKeyValue('')
              }}
              placeholder={hasUserKey ? 'Enter new key to replace' : 'Enter your API key (e.g. sk-...)'}
            />
          </FormField>
          <Space>
            <Button
              data-testid="ullm-save-key-button"
              onClick={handleSave}
              loading={savingFor === currentProvider.id || saving}
              disabled={!keyValue.trim() || keyValue === KEY_DISPLAY_PLACEHOLDER}
            >
              {hasUserKey ? 'Update Key' : 'Save Key'}
            </Button>
            {hasUserKey && (
              <Button
                data-testid="ullm-remove-key-button"
                variant="destructive"
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
                  data-testid="ullm-provider-dropdown"
                  items={menuItems}
                  onSelect={(key) => setSelectedId(key)}
                  align="start"
                >
                  <Button className="w-fit" size="lg" data-testid="ullm-provider-dropdown-trigger">
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
