import { useState } from 'react'
import { Button, Dropdown, theme, message as antMessage } from 'antd'
import { SendOutlined, PlusOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ExtensionSlot, chatExtensionRegistry } from '@/modules/chat/core/extensions'
import { PlusDropdownContext } from '@/modules/chat/components/PlusDropdownContext'
import { EditingMessageBanner } from '@/modules/chat/components/EditingMessageBanner'

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
  const [focused, setFocused] = useState(false)
  const [plusOpen, setPlusOpen] = useState(false)

  // Get stores
  const { sendMessage, sending, isStreaming } = Stores.Chat

  // Extensions can block the Send button via `useSendBlocker`. File's
  // chat-extension uses this to gate Send while uploads are in flight
  // — chat itself stays file-agnostic. Click-time defense lives in the
  // `beforeSendMessage` aggregator (called inside sendMessage).
  const sendBlockers = chatExtensionRegistry.useSendBlockers()
  const isBlockedByExtension = sendBlockers.length > 0

  const handleSend = async () => {
    if (sending || isStreaming || disabled || isBlockedByExtension) return

    try {
      // sendMessage auto-creates conversation if missing
      // Text extension validates content via beforeSendMessage
      // NewChatPage handles navigation via useEffect
      await sendMessage()
    } catch (error: any) {
      console.error('Failed to send message:', error)
      antMessage.error(error.message || 'Failed to send message')
    }
  }

  return (
    <div className={`w-full relative ${className}`} style={style}>
      <div
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        style={{
          border: `1px solid ${focused ? token.colorPrimary : token.colorBorderSecondary}`,
          borderRadius: token.borderRadiusLG,
          backgroundColor: token.colorBgContainer,
          transition: 'border-color 0.2s, box-shadow 0.2s',
          boxShadow: focused ? `0 0 0 2px ${token.colorPrimaryBg}` : undefined,
        }}
      >
        {/* Edit mode indicator — shown when user is editing an existing message */}
        <EditingMessageBanner />

        {/* Input area */}
        <div style={{ padding: '10px 12px 4px' }}>
          {/* Extension slot: input area prefix (file previews, etc.) */}
          <ExtensionSlot name="input_area_prefix" />

          {/* Extension slot: main text input */}
          <ExtensionSlot name="text_input" />

          {/* Extension slot: input area suffix */}
          <ExtensionSlot name="input_area_suffix" />
        </div>

        {/* Toolbar */}
        <div
          style={{
            padding: '4px 8px 8px',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}
        >
          {/* Left: + dropdown + other toolbar actions */}
          <div className="flex items-center gap-1">
            <Dropdown
              open={plusOpen}
              onOpenChange={setPlusOpen}
              trigger={['click']}
              popupRender={() => (
                <PlusDropdownContext.Provider value={{ close: () => setPlusOpen(false) }}>
                  <div
                    style={{
                      backgroundColor: token.colorBgContainer,
                      borderRadius: token.borderRadiusLG,
                      boxShadow: token.boxShadowSecondary,
                      padding: 4,
                    }}
                  >
                    <ExtensionSlot name="toolbar_plus_items" className="flex flex-col" />
                  </div>
                </PlusDropdownContext.Provider>
              )}
            >
              <Button
                icon={<PlusOutlined style={{ fontSize: 16 }} />}
                type="text"
                size="large"
                aria-label="Add attachment"
              />
            </Dropdown>
            <ExtensionSlot name="toolbar_actions" className="flex items-center gap-1" />
          </div>

          {/* Right: model selector + send button */}
          <div className="flex items-center gap-2">
            <ExtensionSlot name="toolbar_model" />
            <Button
              type="primary"
              size="large"
              icon={<SendOutlined rotate={270} />}
              onClick={handleSend}
              disabled={sending || isStreaming || disabled || isBlockedByExtension}
              loading={sending || isStreaming || isBlockedByExtension}
              aria-label="Send message"
            />
          </div>
        </div>
        {/* Status row: active MCP servers + selected assistant */}
        <ExtensionSlot
          name="toolbar_status"
          className="flex flex-wrap items-center gap-1.5 px-3 pb-2 empty:hidden"
        />
      </div>
    </div>
  )
}
