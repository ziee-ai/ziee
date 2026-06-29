import { useState } from 'react'
import { Button, Popover, message } from '@/components/ui'
import { Plus, Send as SendIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
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
      message.error(error.message || 'Failed to send message')
    }
  }

  return (
    <div className={`w-full relative ${className}`} style={style}>
      <div
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        className={cn(
          'rounded-lg bg-card border transition-colors',
          focused ? 'border-primary ring-2 ring-accent' : 'border-border',
        )}
      >
        {/* Edit mode indicator — shown when user is editing an existing message */}
        <EditingMessageBanner />

        {/* Input area */}
        <div className="px-3 pt-2.5 pb-1">
          {/* Extension slot: input area prefix (file previews, etc.) */}
          <ExtensionSlot name="input_area_prefix" />

          {/* Extension slot: main text input */}
          <ExtensionSlot name="text_input" />

          {/* Extension slot: input area suffix */}
          <ExtensionSlot name="input_area_suffix" />
        </div>

        {/* Toolbar */}
        <div className="flex justify-between items-center px-2 pt-1 pb-2">
          {/* Left: + dropdown + other toolbar actions */}
          <div className="flex items-center gap-1">
            <Popover
              open={plusOpen}
              onOpenChange={setPlusOpen}
              align="start"
              side="top"
              content={
                <PlusDropdownContext.Provider value={{ close: () => setPlusOpen(false) }}>
                  <ExtensionSlot name="toolbar_plus_items" className="flex flex-col" />
                </PlusDropdownContext.Provider>
              }
            >
              <Button
                data-testid="chat-input-add-btn"
                icon={<Plus className="size-4" />}
                variant="ghost"
                size="lg"
                aria-label="Add attachment"
              />
            </Popover>
            <ExtensionSlot name="toolbar_actions" className="flex items-center gap-1" />
          </div>

          {/* Right: model selector + send button */}
          <div className="flex items-center gap-2">
            <ExtensionSlot name="toolbar_model" />
            <Button
              data-testid="chat-input-send-btn"
              size="lg"
              icon={<SendIcon className="rotate-[270deg]" />}
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
