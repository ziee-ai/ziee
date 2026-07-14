import { useState } from 'react'
import { Button, Space, Tooltip, message } from '@/components/ui'
import { Copy as CopyIcon, Pencil, RotateCw } from 'lucide-react'
import { Stores } from '@/core/stores'
import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'

/**
 * Core component rendered via MessageContext in ChatMessage.
 * Reads the current message from MessageContext (provided by ChatMessage).
 *
 * - User messages:      Edit button → populates Chat Input with message data
 * - Assistant messages: Regenerate button → auto-sends on a new branch
 * - Both:               Copy button
 */
export function MessageActions() {
  const msg = useMessageContext()
  const [isRegenerating, setIsRegenerating] = useState(false)

  // Bind to THIS pane's store (ITEM-38): edit/regenerate are actions that would
  // otherwise route to the FOCUSED pane; on a same-conversation split that
  // regenerates on the wrong pane. Captured once so it can't drift across awaits.
  const chat = (useChatPaneOrNull()?.store ?? Stores.Chat) as typeof Stores.Chat
  const { isStreaming, sending } = chat

  if (!msg) return null

  const isUser = msg.role === 'user'
  const isAssistant = msg.role === 'assistant'
  const isBusy = isStreaming || sending

  /** Extract plain text from a message's contents */
  const extractText = () => {
    for (const content of msg.contents) {
      const data = content.content as any
      if (data?.type === 'text' && typeof data.text === 'string') {
        return data.text
      }
    }
    return ''
  }

  /** Copy message text to clipboard */
  const handleCopy = async () => {
    const text = extractText()
    if (!text) return
    try {
      await navigator.clipboard.writeText(text)
      message.success('Copied!')
    } catch {
      message.error('Failed to copy')
    }
  }

  /**
   * Enter edit mode: populates the Chat Input with the message's full content
   * (text + files). Extensions subscribe to the editingMessage field via
   * useChatStore.subscribe() in their initialize() hooks to restore their state.
   */
  const handleEdit = () => {
    chat.startEditMessage(msg.id)
  }

  /**
   * Regenerate an assistant response on a new branch.
   * Delegates to Chat.store.startRegenerateMessage which:
   * 1. Finds the preceding user message
   * 2. Pre-fills TextStore with that text
   * 3. Sets pending branch state
   * 4. Auto-sends
   */
  const handleRegenerate = async () => {
    if (isRegenerating || isBusy) return
    setIsRegenerating(true)

    try {
      await chat.startRegenerateMessage(msg.id)
    } catch (err: any) {
      message.error(err?.message || 'Regenerate failed')
    } finally {
      setIsRegenerating(false)
    }
  }

  return (
    <Space
      size={2}
      // hover-none:opacity-100 — on touch / non-hover devices there's no hover to
      // reveal the actions, so keep them always visible.
      className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-within:opacity-100 hover-none:opacity-100 transition-opacity"
    >
      <Tooltip content="Copy">
        <Button
          data-testid="chat-message-copy-btn"
          variant="ghost"
          size="default"
          icon={<CopyIcon />}
          onClick={handleCopy}
        />
      </Tooltip>

      {isUser && (
        <Tooltip content="Edit message">
          <Button
            variant="ghost"
            size="default"
            icon={<Pencil />}
            disabled={isBusy}
            onClick={handleEdit}
            data-testid="edit-message-button"
          />
        </Tooltip>
      )}

      {isAssistant && (
        <Tooltip content="Regenerate response">
          <Button
            variant="ghost"
            size="default"
            icon={<RotateCw />}
            loading={isRegenerating}
            disabled={isBusy && !isRegenerating}
            onClick={handleRegenerate}
            data-testid="regenerate-button"
          />
        </Tooltip>
      )}
    </Space>
  )
}
