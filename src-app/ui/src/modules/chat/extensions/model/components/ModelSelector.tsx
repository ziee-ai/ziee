import { useMemo } from 'react'
import { Select } from 'antd'
import { Stores } from '@/core/stores'

/**
 * ModelSelector Component
 * Self-contained model selection dropdown
 *
 * Features:
 * - Computes available models from providers on-demand
 * - Manages selected model via ModelStore.setModelId()
 * - No props needed - fully self-contained
 */
export function ModelSelector() {
  // Read state from stores
  const { selectedModelId, providers } = Stores.Chat.ModelStore
  const { sending } = Stores.Chat

  // Compute available models from providers
  const availableModels = useMemo(() => {
    const modelGroups: Array<{
      label: string
      options: Array<{ label: string; value: string; description?: string }>
    }> = []

    providers.forEach(provider => {
      if (provider.llm_models && provider.llm_models.length > 0) {
        const enabledModels = provider.llm_models.filter(model => model.enabled)

        if (enabledModels.length > 0) {
          modelGroups.push({
            label: provider.name,
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
    Stores.Chat.ModelStore.setModelId(value)
  }

  return (
    <div data-testid="model-selector">
      <Select
        value={selectedModelId}
        onChange={handleChange}
        popupMatchSelectWidth={false}
        placeholder="Select Model"
        disabled={sending}
        options={availableModels}
        style={{ minWidth: 120, fontSize: 15 }}
        variant="borderless"
      />
    </div>
  )
}
