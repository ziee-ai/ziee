import { useState } from 'react'
import { Button, Popover, Tooltip, message } from '@/components/ui'
import { Plus, SendHorizontal as SendIcon } from 'lucide-react'
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
    <div className={`w-full relative ${className}`} style={style} data-chat-composer>

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

        {/* Toolbar — the left (secondary) group yields space first so the right
            send group is never clipped on narrow widths (chat panel or mobile). */}
        <div className="flex justify-between items-center gap-2 px-2 pt-1 pb-2">
          {/* Left: + dropdown + other toolbar actions. `min-w-0 flex-1` lets the
              keyboard-tips text truncate instead of pushing Send off the edge. */}
          <div className="flex items-center gap-1 min-w-0 flex-1">
            {/* Tooltip anchors to the wrapper span (a distinct DOM node), not
                the Popover-trigger button — two triggers on ONE node thrash and
                flicker. The button suppresses its own aria-label auto-tooltip via
                data-tooltip-wrapped so only the span's tooltip shows. */}
            <Tooltip content="Add tools & files">
              <span className="inline-flex shrink-0">
                <Popover
                  open={plusOpen}
                  onOpenChange={setPlusOpen}
                  align="start"
                  side="top"
                  className="w-auto"
                  content={
                    <PlusDropdownContext.Provider value={{ close: () => setPlusOpen(false) }}>
                      <ExtensionSlot name="toolbar_plus_items" className="flex flex-col" />
                    </PlusDropdownContext.Provider>
                  }
                >
                  <Button
                    data-testid="chat-input-add-btn"
                    data-tooltip-wrapped=""
                    icon={<Plus className="size-4" />}
                    variant="ghost"
                    size="default"
                    aria-label="Add tools & files"
                  />
                </Popover>
              </span>
            </Tooltip>
            <ExtensionSlot name="toolbar_actions" className="flex items-center gap-1 min-w-0" />
          </div>

          {/* Right: model selector + send button. `shrink-0` keeps Send fully
              visible; the model selector caps its own width internally. */}
          <div className="flex items-center gap-2 shrink-0">
            <ExtensionSlot name="toolbar_model" />
            <Button
              data-testid="chat-input-send-btn"
              size="default"
              icon={<SendIcon className="-rotate-90" />}
              onClick={handleSend}
              disabled={sending || isStreaming || disabled || isBlockedByExtension}
              loading={sending || isStreaming || isBlockedByExtension}
              aria-label="Send message"
            />
          </div>
        </div>
        {/* Status row: active MCP servers + selected assistant. An intentional
            variable-length wrap row (chips count depends on active tools/pills);
            the testid gives the geometry audit a stable target so its B1
            near-fit wrap can be allow-listed precisely (not by class substring).
            pb-3 (not pb-2): the outlined status chips (memory / summary pills)
            are the composer card's last row, so their bottom border sat only 8px
            from the card's own border — a crowded double stroke (A12). 12px of
            breathing room clears the double-border without ghosting the pills. */}
        <ExtensionSlot
          name="toolbar_status"
          data-testid="composer-status-slot"
          className="flex flex-wrap items-center gap-1.5 px-3 pb-3 empty:hidden"
        />
      </div>
    </div>
  )
}
