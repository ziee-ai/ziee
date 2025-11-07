import { PlusOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Dropdown,
  Empty,
  Flex,
  Menu,
  Spin,
  Typography,
} from 'antd'
import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  Stores,
} from '../stores'
import { PROVIDER_ICONS } from '../constants'
import { LlmProviderDrawer } from './LlmProviderDrawer'
import { LocalProviderSettings } from './LocalProviderSettings'
import { RemoteProviderSettings } from './RemoteProviderSettings'
import { useWindowMinSize } from '@/hooks/useWindowMinSize'
import { IoIosArrowDown } from 'react-icons/io'

const { Title } = Typography

export function LlmProviderSettings() {
  const { message } = App.useApp()
  const { providerId } = useParams<{ providerId?: string }>()
  const navigate = useNavigate()
  const windowMinSize = useWindowMinSize()

  // Provider store
  const { providers, loading, error } = Stores.LlmProvider

  const currentProvider = providers.find(p => p.id === providerId)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.LlmProvider.clearLlmProviderStoreError()
    }
  }, [error, message])

  // Handle URL parameter and provider selection
  useEffect(() => {
    if (providers.length > 0) {
      if (providerId) {
        // If URL has providerId, check if it's valid
        const providerExists = providers.find(p => p.id === providerId)
        if (!providerExists) {
          // Provider doesn't exist, redirect to first provider
          navigate(`/settings/llm-providers/${providers[0].id}`, {
            replace: true,
          })
        }
      } else {
        // No URL parameter, navigate to first provider
        navigate(`/settings/llm-providers/${providers[0].id}`, {
          replace: true,
        })
      }
    }
  }, [providers, providerId, navigate])

  const menuItems = providers.map(provider => {
    const IconComponent =
      PROVIDER_ICONS[provider.provider_type] || PROVIDER_ICONS.custom
    return {
      key: provider.id,
      label: (
        <Flex className={'flex-row gap-2 items-center h-full'}>
          <IconComponent className={'text-lg'} />
          <div className={'flex-1 flex items-center h-full overflow-x-hidden'}>
            <Typography.Text ellipsis>{provider.name}</Typography.Text>
          </div>
        </Flex>
      ),
    }
  })

  menuItems.push({
    key: 'add-provider',
    //@ts-ignore
    icon: <PlusOutlined />,
    label: <Typography.Text>Add Provider</Typography.Text>,
  })

  const ProviderMenu = () => (
    <Menu
      className={`
      w-full
      h-full
      !m-0
      [&_.ant-menu]:!px-0
      [&_.ant-menu-item]:!h-8
      [&_.ant-menu-item]:!leading-[32px]
      !bg-transparent
      !border-none`}
      selectedKeys={providerId ? [providerId] : []}
      items={menuItems}
      onClick={({ key }) => {
        if (key === 'add-provider') {
          Stores.LlmProviderDrawer.openLlmProviderDrawer()
        } else {
          navigate(`/settings/llm-providers/${key}`)
        }
      }}
    />
  )

  const renderProviderSettings = () => {
    if (loading) {
      return (
        <div style={{ textAlign: 'center', padding: '50px' }}>
          <Spin size="large" />
        </div>
      )
    }

    if (!currentProvider) {
      return (
        <Empty
          description={'No provider selected'}
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      )
    }

    // Render appropriate provider settings component based on type
    console.log('[LlmProviderSettings] Provider type:', currentProvider.provider_type)
    if (currentProvider.provider_type === 'local') {
      console.log('[LlmProviderSettings] Rendering LocalProviderSettings')
      return <LocalProviderSettings />
    }

    console.log('[LlmProviderSettings] Rendering RemoteProviderSettings')
    return <RemoteProviderSettings />
  }

  return (
    <div className="flex flex-col gap-3 h-full overflow-y-hidden">
      <div className={'flex w-full h-full flex-1 relative justify-center'}>
        <div className={'w-full h-full flex self-center'}>
          {!windowMinSize.sm && (
            <div className={'w-42 flex flex-col gap-2 h-full pt-3'}>
              <div className={'w-full px-3'}>
                <Title level={4} className="!m-0 !leading-tight">
                  Providers
                </Title>
              </div>
              <div className={'flex-1 pl-2 overflow-y-auto'}>
                <ProviderMenu />
              </div>
            </div>
          )}
          {/* Main Content */}
          <div className={'flex flex-1'}>
            <div className={'flex w-full flex-col py-3 px-3 overflow-y-auto'}>
              <div
                className={'flex flex-col flex-1 max-w-3xl self-center w-full'}
              >
                {windowMinSize.sm && (
                  <div
                    className={'w-full flex flex-row gap-2 items-center mb-4'}
                  >
                    <Dropdown
                      className={'w-full'}
                      menu={{
                        items: menuItems.map(item => ({
                          // @ts-ignore
                          icon: item.icon,
                          key: item.key,
                          label: item.label,
                        })),
                        onClick: ({ key }) => {
                          if (key === 'add-provider') {
                            Stores.LlmProviderDrawer.openLlmProviderDrawer()
                          } else {
                            navigate(`/settings/llm-providers/${key}`)
                          }
                        },
                        selectedKeys: providerId ? [providerId] : [],
                      }}
                      trigger={['click']}
                    >
                      <Button className="w-fit" size={'large'}>
                        {currentProvider ? (
                          <Flex className="gap-2 items-center">
                            {(() => {
                              const IconComponent =
                                PROVIDER_ICONS[currentProvider.provider_type] ||
                                PROVIDER_ICONS.custom
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
                {renderProviderSettings()}
              </div>
              <div className={'w-full h-3 block'} />
            </div>
          </div>
        </div>

        {/* Modals */}
        <LlmProviderDrawer />
      </div>
    </div>
  )
}
