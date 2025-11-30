import { useEffect, useRef, useState } from 'react'
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
 * - Reads available models from ModelStore (computed from ChatLlmProvider)
 * - Manages selected model via ModelStore.setModelId()
 * - Responsive UI (compact on small screens)
 * - No props needed - fully self-contained
 */
export function ModelSelector() {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isBreaking, setIsBreaking] = useState<boolean>(false)

  // Read state from stores
  const { selectedModelId, availableModels } = Stores.Chat.ModelStore
  const { sending } = Stores.Chat

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
