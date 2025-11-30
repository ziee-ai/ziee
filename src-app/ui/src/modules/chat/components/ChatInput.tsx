import {
  Button,
  Card,
  Flex,
  theme,
  message as antMessage,
} from 'antd'
import { SendOutlined } from '@ant-design/icons'
import { useNavigate, useParams } from 'react-router-dom'
import { ModelSelector } from '../extensions/model/components/ModelSelector'
import { Stores } from '@/core/stores'
import { ExtensionSlot } from '../core/extensions'

interface ChatInputProps {
  disabled?: boolean
  className?: string
  style?: React.CSSProperties
}

/**
 * ChatInput Component
 * Orchestrates message sending using extension stores
 */
export function ChatInput({
  disabled = false,
  className = '',
  style,
}: ChatInputProps) {
  const { token } = theme.useToken()
  const navigate = useNavigate()
  const { conversationId } = useParams<{ conversationId?: string }>()

  // Get stores
  const { createConversation, sendMessage, sending, isStreaming } = Stores.Chat

  const handleSend = async () => {
    if (sending || isStreaming || disabled) return

    // Validate text from TextStore
    const messageToSend = Stores.Chat.TextStore.getText()
    if (!messageToSend?.trim()) {
      antMessage.error('Message cannot be empty')
      return
    }

    try {
      if (conversationId) {
        // Existing conversation - just send message
        // Model extension will provide model_id via composeRequestFields
        await sendMessage()
      } else {
        // New conversation - create WITHOUT model_id (backend auto-updates)
        const conversation = await createConversation()
        await sendMessage()
        navigate(`/chat/${conversation.id}`)
      }
    } catch (error: any) {
      console.error('Failed to send message:', error)
      antMessage.error(error.message || 'Failed to send message')
    }
  }

  return (
    <div className={`w-full relative ${className}`} style={style}>
      <Card
        classNames={{ body: '!p-0' }}
        style={{
          borderColor: token.colorBorderSecondary,
          transition: 'border-color 0.2s, box-shadow 0.2s',
          backgroundColor: token.colorBgContainer,
        }}
      >
        <div style={{ padding: '8px' }}>
          <Flex className="flex-col gap-3 w-full">
            {/* Extension slot: input area prefix */}
            <ExtensionSlot name="input_area_prefix" />

            {/* Extension slot: main text input */}
            <ExtensionSlot name="text_input" />

            {/* Extension slot: input area suffix */}
            <ExtensionSlot name="input_area_suffix" />

            <div className="w-full flex justify-between gap-0">
              <div className="flex gap-1">
                {/* Extension slot: toolbar actions (file upload, tools, etc.) */}
                <ExtensionSlot name="toolbar_actions" />
              </div>

              <div className={'flex items-center gap-[6px]'}>
                <ModelSelector />

                <div className={'items-center justify-end gap-1 flex'}>
                  <Button
                    type="primary"
                    icon={<SendOutlined rotate={270} />}
                    onClick={handleSend}
                    disabled={sending || disabled}
                    loading={sending}
                    aria-label="Send message"
                  />
                </div>
              </div>
            </div>
          </Flex>
        </div>
      </Card>
    </div>
  )
}
