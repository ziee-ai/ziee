import { useState, useEffect } from 'react'
import { useApiKeysStepStore } from './apiKeysStep'
import {
  Spin,
  ErrorState,
  Button,
  Tag,
  Flex,
  Text,
  Title,
  Paragraph,
  PasswordInput,
} from '@ziee/kit'
import { Plug, CircleCheck } from 'lucide-react'
import type { OnboardingStepProps } from '@/modules/onboarding/types/onboarding'
import { Stores } from '@ziee/framework/stores'
import { PROVIDER_ICONS } from '@/modules/llm-provider/constants'

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
  }, [providers, selectedId])

  useEffect(() => {
    Stores.Onboarding.setReady(true)
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
        <Spin label="Loading" />
      </div>
    )
  }

  // A load failure must not masquerade as the "no providers enabled" empty
  // state — surface it with a retry so setup isn't silently blocked.
  if (error && providers.length === 0) {
    return (
      <div className="max-w-lg" data-testid="onboarding-apikeys-error">
        <div className="flex items-center gap-3 mb-4">
          <Plug className="text-3xl text-primary" />
          <Title level={3} className="!mb-0">
            AI Providers
          </Title>
        </div>
        <ErrorState
          resource="AI providers"
          description="The available AI providers couldn't be loaded."
          details={error}
          onRetry={() => Stores.ApiKeysStep.loadProviders()}
          data-testid="onboarding-apikeys-error-alert"
        />
      </div>
    )
  }

  if (providers.length === 0) {
    return (
      <div className="max-w-lg" data-testid="onboarding-apikeys-empty">
        <div className="flex items-center gap-3 mb-4">
          <Plug className="text-3xl text-primary" />
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
            <Text ellipsis>{provider.name}</Text>
          </div>
        </Flex>
      ),
    }
  })

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-3 mb-3">
        <Plug className="text-2xl text-primary" />
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
        <div className="mb-3">
          <ErrorState
            resource="AI providers"
            description="Couldn't refresh the provider list."
            details={error}
            onRetry={() => Stores.ApiKeysStep.loadProviders()}
            data-testid="onboarding-apikeys-refresh-error-alert"
          />
        </div>
      )}

      {/* Two-column layout */}
      <div className="flex flex-1 mb-4">
        {/* Left sidebar */}
        <div className="w-40 flex-shrink-0 pt-1">
          <div className="w-full h-full">
            {menuItems.map(item => (
              <Button
                key={item.key}
                variant="ghost"
                block
                data-testid={`onboarding-apikeys-nav-${item.key}`}
                onClick={() => setSelectedId(item.key)}
                className={`justify-start px-2 h-8 ${
                  currentProvider.id === item.key
                    ? 'bg-accent text-accent-foreground'
                    : 'hover:bg-muted text-foreground'
                }`}
              >
                {item.label}
              </Button>
            ))}
          </div>
        </div>

        {/* Right content */}
        <div className="flex-1 px-4">
          <Flex align="center" gap="small" className="mb-1">
            {(() => {
              const IconComponent = PROVIDER_ICONS[currentProvider.provider_type] || PROVIDER_ICONS.custom
              return <IconComponent className="text-xl" />
            })()}
            <Text strong className="text-base">{currentProvider.name}</Text>
            {(currentProvider.api_key_configured || hasUserKey) && (
              <Tag variant="outline" data-testid="onboarding-apikeys-key-status-tag" icon={<CircleCheck />} tone="success">
                {hasUserKey ? 'Your key configured' : 'Admin key configured'}
              </Tag>
            )}
          </Flex>

          <Text type="secondary" className="block text-xs mb-4">
            {currentProvider.api_key_configured
              ? 'Enter your own key to override the admin key.'
              : 'No system key is set. Enter your own to use this provider.'}
          </Text>

          <div className="max-w-sm">
            <div className="mb-2">
              <label className="block text-sm font-medium mb-1">Your API Key</label>
              <PasswordInput
                data-testid="onboarding-apikeys-password-input"
                showLabel="Show API key"
                hideLabel="Hide API key"
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
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
