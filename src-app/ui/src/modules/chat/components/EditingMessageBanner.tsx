import { Button, Tooltip } from '@/components/ui'
import { Text } from '@/components/ui'
import { Pencil, X } from 'lucide-react'
import { Stores } from '@/core/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

/**
 * Shows a banner above the Chat Input when the user is in edit mode.
 * Displays a "Editing message" label and a Cancel button.
 *
 * Rendered by ChatInput whenever Stores.Chat.editingMessage is non-null.
 * Clicking Cancel calls cancelEdit() which clears editingMessage, restores
 * trimmed messages, and clears the text input.
 */
export function EditingMessageBanner() {
  // Bind to THIS pane's store (audit #10): the edit is started on the pane's own
  // store (MessageActions), so the banner + cancel must read/act on that pane, not
  // the focused-pane bridge.
  const chat = (useChatPaneOrNull()?.store ?? Stores.Chat) as typeof Stores.Chat
  const editingMessage = chat.editingMessage

  if (!editingMessage) return null

  return (
    <div
      data-testid="chat-editing-banner"
      className="flex items-center justify-between px-3 py-1.5 border-b border-border bg-muted/40 rounded-lg rounded-b-none"
    >
      <div className="flex items-center gap-1.5">
        <Pencil className="text-xs text-muted-foreground" />
        <Text type="secondary" className="text-xs">
          Editing message
        </Text>
      </div>
      <Tooltip content="Cancel edit">
        <Button
          data-testid="chat-editing-cancel-btn"
          variant="ghost"
          size="default"
          icon={<X />}
          onClick={() => chat.cancelEdit()}
          aria-label="Cancel edit"
        />
      </Tooltip>
    </div>
  )
}
