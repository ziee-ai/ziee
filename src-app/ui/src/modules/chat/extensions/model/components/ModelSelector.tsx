import { useEffect, useRef, useState, useMemo } from 'react'
import { Select, Button } from 'antd'
import { SettingOutlined } from '@ant-design/icons'
import { IoIosArrowDown } from 'react-icons/io'
import { Stores } from '@/core/stores'

const UI_BREAKPOINT = 480

const calculateIsBreaking = (width: number): boolean => width <= UI_BREAKPOINT

/**
 * ModelSelector Component
 * Self-contained model selection dropdown
 *
 * Features:
 * - Computes available models from providers on-demand
 * - Manages selected model via ModelStore.setModelId()
 * - Responsive UI (compact on small screens)
 * - No props needed - fully self-contained
 */
export function ModelSelector() {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isBreaking, setIsBreaking] = useState<boolean>(false)

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
              value: model.id, // Just the model ID - UUIDs are globally unique
              description: model.description,
            })),
          })
        }
      }
    })

    return modelGroups
  }, [providers])

  // Handle responsive breakpoint
  useEffect(() => {
    const containerElement = containerRef.current
    if (!containerElement) return

    const updateBreaking = (width: number) => {
      setIsBreaking(calculateIsBreaking(width))
    }

    updateBreaking(containerElement.offsetWidth)

    const resizeObserver = new ResizeObserver(entries => {
      for (const entry of entries) {
        updateBreaking(entry.contentRect.width)
      }
    })

    resizeObserver.observe(containerElement)

    return () => resizeObserver.disconnect()
  }, [])

  const handleChange = (value: string) => {
    Stores.Chat.ModelStore.setModelId(value)
  }

  return (
    <div ref={containerRef} style={{ display: 'inline-block' }}>
      <Select
        value={selectedModelId}
        onChange={handleChange}
        popupMatchSelectWidth={false}
        placeholder="Select Model"
        disabled={sending}
        options={availableModels}
        style={{ width: isBreaking ? 40 : 120 }}
        variant={isBreaking ? 'borderless' : undefined}
        labelRender={isBreaking ? () => '' : undefined}
        prefix={
          isBreaking && (
            <Button>
              <SettingOutlined />
            </Button>
          )
        }
        suffixIcon={<IoIosArrowDown />}
      />
    </div>
  )
}
