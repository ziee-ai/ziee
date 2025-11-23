import { useEffect, useMemo, useRef, useState } from 'react'
import { Button, Card, Flex, Form, Input, theme } from 'antd'
import { SendOutlined } from '@ant-design/icons'
import { ModelSelector } from './ModelSelector'
import { Stores } from '@/core/stores'
import type { LlmProviderWithModels } from '@/modules/llm-provider/stores/LlmProvider.store'
import type { LlmModel } from '@/api-client/types'

const { TextArea } = Input

const UI_BREAKPOINT = 480

const calculateIsBreaking = (width: number): boolean => width <= UI_BREAKPOINT

interface ChatInputProps {
  onSend: (content: string, modelId: string) => void
  disabled?: boolean
  loading?: boolean
  placeholder?: string
  defaultModelId?: string
  className?: string
  style?: React.CSSProperties
}

export function ChatInput({
  onSend,
  disabled = false,
  loading = false,
  placeholder = 'Message...',
  defaultModelId,
  className = '',
  style,
}: ChatInputProps) {
  const [form] = Form.useForm()
  const { token } = theme.useToken()
  const [isBreaking, setIsBreaking] = useState<boolean>(false)
  const [isFocused, setIsFocused] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  // Get available models from ChatLlmProvider store (user-scoped)
  const { providers } = Stores.ChatLlmProvider

  // Build available models list
  const availableModels = useMemo(() => {
    const modelGroups: Array<{
      label: string
      options: Array<{ label: string; value: string; description?: string }>
    }> = []

    providers.forEach((provider: LlmProviderWithModels) => {
      if (provider.llm_models && provider.llm_models.length > 0) {
        // Only include enabled and active models
        const enabledModels = provider.llm_models.filter(
          (model: LlmModel) => model.enabled && model.is_active
        )

        if (enabledModels.length > 0) {
          modelGroups.push({
            label: provider.name,
            options: enabledModels.map((model: LlmModel) => ({
              label: model.display_name || model.name,
              value: `${provider.id}:${model.id}`,
              description: model.description,
            })),
          })
        }
      }
    })

    return modelGroups
  }, [providers])

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

  useEffect(() => {
    if (
      !form.getFieldValue('model') &&
      availableModels.length > 0 &&
      availableModels[0].options.length > 0
    ) {
      form.setFieldValue('model', availableModels[0].options[0].value)
    }
  }, [availableModels, form])

  useEffect(() => {
    if (defaultModelId) {
      // Find matching model in format "providerId:modelId"
      for (const providerGroup of availableModels) {
        const matchingModel = providerGroup.options.find(model =>
          model.value.endsWith(`:${defaultModelId}`)
        )
        if (matchingModel) {
          form.setFieldValue('model', matchingModel.value)
          break
        }
      }
    }
  }, [defaultModelId, availableModels, form])

  const handleSend = async () => {
    if (loading || disabled) return

    const formValues = form.getFieldsValue()
    const { message: messageToSend, model: selectedModel } = formValues

    if (!messageToSend?.trim()) {
      return
    }

    if (!selectedModel) {
      return
    }

    form.setFieldValue('message', '')

    const [, modelId] = selectedModel.split(':')
    onSend(messageToSend.trim(), modelId)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div
      ref={containerRef}
      className={`w-full relative ${className}`}
      style={style}
    >
      <Card
        classNames={{ body: '!p-0' }}
        style={{
          borderColor: isFocused
            ? token.colorPrimaryBorder
            : token.colorBorderSecondary,
          transition: 'border-color 0.2s, box-shadow 0.2s',
          backgroundColor: token.colorBgContainer,
        }}
      >
        <Form
          form={form}
          layout="vertical"
          className="w-full"
          initialValues={{
            message: '',
            model: undefined,
          }}
          disabled={disabled}
        >
          <div style={{ padding: '8px' }}>
            <Flex className="flex-col gap-3 w-full">
              <div className="w-full">
                <Form.Item name="message" className="mb-0" noStyle>
                  <TextArea
                    onKeyDown={handleKeyDown}
                    onFocus={() => setIsFocused(true)}
                    onBlur={() => setIsFocused(false)}
                    placeholder={placeholder}
                    autoSize={{ minRows: 1, maxRows: 6 }}
                    disabled={disabled}
                    className="resize-none !border-none focus:!border-none focus:!outline-none focus:!shadow-none !pt-1"
                    style={{ backgroundColor: 'transparent' }}
                  />
                </Form.Item>
              </div>
              <div className="w-full flex justify-between gap-0">
                <div className="flex gap-1">
                  {/* Placeholder for file upload and tools - not implemented */}
                </div>

                <div className={'flex items-center gap-[6px]'}>
                  <ModelSelector
                    isBreaking={isBreaking}
                    isDisabled={disabled}
                    availableModels={availableModels}
                  />

                  <div className={'items-center justify-end gap-1 flex'}>
                    <Button
                      type="primary"
                      icon={<SendOutlined rotate={270} />}
                      onClick={handleSend}
                      disabled={loading || disabled}
                      loading={loading}
                    />
                  </div>
                </div>
              </div>
            </Flex>
          </div>
        </Form>
      </Card>
    </div>
  )
}
