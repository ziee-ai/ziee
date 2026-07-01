import { Button, Tooltip } from '@/components/ui'
import { Text } from '@/components/ui'
import { Pencil, X } from 'lucide-react'
import { Stores } from '@/core/stores'

/**
 * Shows a banner above the Chat Input when the user is in edit mode.
 * Displays a "Editing message" label and a Cancel button.
 *
 * Rendered by ChatInput whenever Stores.Chat.editingMessage is non-null.
 * Clicking Cancel calls cancelEdit() which clears editingMessage, restores
 * trimmed messages, and clears the text input.
 */
export function EditingMessageBanner() {
  const editingMessage = Stores.Chat.editingMessage

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
          onClick={() => Stores.Chat.__state.cancelEdit()}
          aria-label="Cancel edit"
        />
      </Tooltip>
    </div>
  )
}
