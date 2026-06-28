import { Plus } from 'lucide-react'
import {
  Button,
  Dropdown,
  Empty,
  Text,
  Title,
  Flex,
  message,
} from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { Stores } from '@/modules/llm-provider/stores'
import { DivScrollY } from '@/components/common/DivScrollY'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'
import { LlmProviderDrawer } from '@/modules/llm-provider/components/LlmProviderDrawer'
import { LocalProviderSettings } from '@/modules/llm-provider/components/LocalProviderSettings'
import { RemoteProviderSettings } from '@/modules/llm-provider/components/RemoteProviderSettings'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { IoIosArrowDown } from 'react-icons/io'

export function LlmProviderSettings() {
  const { providerId } = useParams<{ providerId?: string }>()
  const navigate = useNavigate()
  const windowMinSize = useWindowMinSize()

  // Provider store
  const { providers, loading, error } = Stores.LlmProvider
  const canCreate = usePermission(Permissions.LlmProvidersCreate)

  const currentProvider = providers.find(p => p.id === providerId)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.LlmProvider.clearLlmProviderStoreError()
    }
  }, [error])

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
            <Text ellipsis>{provider.name}</Text>
          </div>
        </Flex>
      ),
    }
  })

  if (canCreate) {
    menuItems.push({
      key: 'add-provider',
      label: (
        <Flex className={'flex-row gap-2 items-center h-full'}>
          <Plus className={'text-lg'} />
          <Text>Add Provider</Text>
        </Flex>
      ),
    })
  }

  const ProviderMenu = () => (
    <div
      className={`
      w-full
      h-full
      !m-0
      !px-0
      !bg-transparent
      !border-none
      flex flex-col
      gap-1
    `}
    >
      {menuItems.map(item => {
        const IconComponent =
          item.key !== 'add-provider'
            ? PROVIDER_ICONS[
                providers.find(p => p.id === item.key)?.provider_type || 'custom'
              ] || PROVIDER_ICONS.custom
            : null
        return (
          <button
            key={item.key}
            onClick={() => {
              if (item.key === 'add-provider') {
                Stores.LlmProviderDrawer.openLlmProviderDrawer()
              } else {
                navigate(`/settings/llm-providers/${item.key}`)
              }
            }}
            className={`
              flex items-center gap-2 px-3 py-1.5 rounded-md text-sm w-full text-left
              ${
                providerId === item.key
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50 text-foreground'
              }
              transition-colors
            `}
          >
            {item.key === 'add-provider' ? (
              <span className="text-base">
                <Plus />
              </span>
            ) : (
              IconComponent && <IconComponent className="text-base" />
            )}
            <span className="flex-1 truncate">{item.label}</span>
          </button>
        )
      })}
    </div>
  )

  const renderProviderSettings = () => {
    if (loading) {
      return (
        <Loading />
      )
    }

    if (!currentProvider) {
      return (
        <Empty
          description={'No provider selected'}
          data-testid="llm-provider-settings-empty"
        />
      )
    }

    // Render appropriate provider settings component based on type
    console.log(
      '[LlmProviderSettings] Provider type:',
      currentProvider.provider_type,
    )
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
              <DivScrollY className={'flex-1 pl-2 flex-col'}>
                <ProviderMenu />
              </DivScrollY>
            </div>
          )}
          {/* Main Content */}
          <div className={'flex flex-1'}>
            <DivScrollY className={'flex w-full flex-col py-3 px-3'}>
              <div
                className={'flex flex-col flex-1 max-w-3xl self-center w-full'}
              >
                {windowMinSize.sm && (
                  <div
                    className={'w-full flex flex-row gap-2 items-center mb-4'}
                  >
                    <Dropdown
                      data-testid="llm-provider-select-dropdown"
                      items={menuItems.map(item => ({
                        key: item.key,
                        label: (
                          <Flex className="gap-2 items-center">
                            {item.key === 'add-provider' ? (
                              <Plus />
                            ) : (
                              (() => {
                                const IconComponent =
                                  PROVIDER_ICONS[
                                    providers.find(p => p.id === item.key)?.provider_type || 'custom'
                                  ] || PROVIDER_ICONS.custom
                                return <IconComponent className="text-lg" />
                              })()
                            )}
                            {item.label}
                          </Flex>
                        ),
                      }))}
                      onSelect={(key: string) => {
                        if (key === 'add-provider') {
                          Stores.LlmProviderDrawer.openLlmProviderDrawer()
                        } else {
                          navigate(`/settings/llm-providers/${key}`)
                        }
                      }}
                    >
                      <Button className="w-fit" size={'lg'} data-testid="llm-provider-select-btn">
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
            </DivScrollY>
          </div>
        </div>

        {/* Modals */}
        <LlmProviderDrawer />
      </div>
    </div>
  )
}
