import { useState } from 'react'
import { App, Button, Space, Tooltip } from 'antd'
import { EditOutlined, ReloadOutlined, CopyOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { useMessageContext } from '@/modules/chat/core/MessageContext'

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
  const { message } = App.useApp()
  const [isRegenerating, setIsRegenerating] = useState(false)

  const { isStreaming, sending } = Stores.Chat

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
    Stores.Chat.__state.startEditMessage(msg.id)
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
      await Stores.Chat.__state.startRegenerateMessage(msg.id)
    } catch (err: any) {
      message.error(err?.message || 'Regenerate failed')
    } finally {
      setIsRegenerating(false)
    }
  }

  return (
    <Space
      size={2}
      className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-within:opacity-100 transition-opacity"
    >
      <Tooltip title="Copy">
        <Button
          type="text"
          size="small"
          icon={<CopyOutlined />}
          onClick={handleCopy}
        />
      </Tooltip>

      {isUser && (
        <Tooltip title="Edit message">
          <Button
            type="text"
            size="small"
            icon={<EditOutlined />}
            disabled={isBusy}
            onClick={handleEdit}
            data-testid="edit-message-button"
          />
        </Tooltip>
      )}

      {isAssistant && (
        <Tooltip title="Regenerate response">
          <Button
            type="text"
            size="small"
            icon={<ReloadOutlined />}
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
