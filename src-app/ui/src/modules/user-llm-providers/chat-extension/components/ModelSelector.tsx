import { useState, useMemo } from 'react'
import { Select } from 'antd'
import { WarningOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { ProviderWithModels } from '@/api-client/types'
import { ProviderApiKeyModal } from './ProviderApiKeyModal'
import { useMainContentMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

/**
 * ModelSelector Component
 * Self-contained model selection dropdown
 *
 * Features:
 * - Computes available models from providers on-demand
 * - Manages selected model via ModelStore.setModelId()
 * - Shows a warning icon for non-local providers without an API key configured
 * - Prompts for an API key when selecting a model from a non-local provider
 *   with no key (local providers authenticate via a proxy token — never prompt)
 */

/**
 * Whether a provider still needs an API key before its models can be used.
 *
 * Local providers authenticate via an internal, server-minted proxy token —
 * never a user-supplied API key — so they must never show the warning or
 * trigger the key prompt. Every other provider needs a key unless one is
 * already configured (system- or user-level).
 */
function providerNeedsApiKey(
  provider: Pick<ProviderWithModels, 'provider_type' | 'api_key_configured'>,
): boolean {
  return provider.provider_type !== 'local' && !provider.api_key_configured
}

export function ModelSelector() {
  const { selectedModelId, providers, loading } = Stores.ModelPicker
  const { sending } = Stores.Chat
  const mainContentMinSize = useMainContentMinSize()
  const [pendingProviderForKey, setPendingProviderForKey] = useState<{
    providerId: string
    providerName: string
    modelId: string
  } | null>(null)

  const availableModels = useMemo(() => {
    const modelGroups: Array<{
      label: React.ReactNode
      options: Array<{ label: string; value: string; description?: string }>
    }> = []

    providers.forEach(provider => {
      if (provider.llm_models && provider.llm_models.length > 0) {
        const enabledModels = provider.llm_models.filter(model => model.enabled)

        if (enabledModels.length > 0) {
          const label = providerNeedsApiKey(provider) ? (
            <span className="flex items-center gap-1">
              <WarningOutlined className="text-yellow-500" />
              {provider.name}
            </span>
          ) : (
            provider.name
          )

          modelGroups.push({
            label,
            options: enabledModels.map(model => ({
              label: model.display_name || model.name,
              value: model.id,
              description: model.description,
            })),
          })
        }
      }
    })

    return modelGroups
  }, [providers])

  const handleChange = (value: string) => {
    // Check if selected model belongs to a provider that still needs an API
    // key (local providers never do — they use an internal proxy token).
    for (const provider of providers) {
      if (providerNeedsApiKey(provider) && provider.llm_models) {
        const model = provider.llm_models.find(m => m.id === value)
        if (model) {
          setPendingProviderForKey({
            providerId: provider.id,
            providerName: provider.name,
            modelId: value,
          })
          return
        }
      }
    }
    Stores.ModelPicker.setModelId(value)
  }

  const handleKeyProvided = (modelId: string) => {
    setPendingProviderForKey(null)
    Stores.ModelPicker.setModelId(modelId)
  }

  return (
    <div data-testid="model-selector">
      <Select
        value={selectedModelId}
        onChange={handleChange}
        popupMatchSelectWidth={false}
        placeholder={loading && providers.length === 0 ? 'Loading…' : 'Select Model'}
        aria-label="Model"
        loading={loading && providers.length === 0}
        disabled={sending}
        options={availableModels}
        style={{ fontSize: 15, maxWidth: mainContentMinSize.xs ? 130 : undefined }}
        className="[&_.ant-select-selector]:!w-auto [&_.ant-select-selector]:!min-w-0"
        variant="borderless"
      />
      {pendingProviderForKey && (
        <ProviderApiKeyModal
          providerId={pendingProviderForKey.providerId}
          providerName={pendingProviderForKey.providerName}
          modelId={pendingProviderForKey.modelId}
          onSuccess={handleKeyProvided}
          onCancel={() => setPendingProviderForKey(null)}
        />
      )}
    </div>
  )
}
