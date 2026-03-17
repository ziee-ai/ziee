import { useState } from 'react'
import { App, Button, Space, Tooltip } from 'antd'
import { EditOutlined, ReloadOutlined, CopyOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { useMessageContext } from '@/modules/chat/core/MessageContext'

/**
 * Zero-arg slot component rendered in the 'message_actions' slot.
 * Reads the current message from MessageContext (provided by ChatMessage).
 *
 * - User messages:     Edit button → opens inline editor in the message bubble
 * - Assistant messages: Regenerate button → auto-sends on a new branch
 * - Both:              Copy button
 *
 * While a message is being edited, its bubble shows the InlineEditor instead
 * of this component (ChatMessage handles the conditional render).
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
   * Open the inline editor in the message bubble.
   * ChatMessage reads editingMessageId from BranchingStore and swaps
   * the content area for an InlineEditor when it matches this message.
   */
  const handleEdit = () => {
    const text = extractText()
    Stores.Chat.__state.BranchingStore.startEditing(msg.id, text)
  }

  /**
   * Regenerate an assistant response on a new branch:
   * 1. Find the user message that immediately preceded this assistant message.
   * 2. Pre-fill TextStore with that user message's text.
   * 3. Set the branch point to that user message.
   * 4. Auto-send so the AI generates a new response on the new branch.
   */
  const handleRegenerate = async () => {
    if (isRegenerating || isBusy) return
    setIsRegenerating(true)

    try {
      const sorted = [...Stores.Chat.__state.messages.values()].sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
      )

      const currentIndex = sorted.findIndex(m => m.id === msg.id)
      if (currentIndex <= 0) {
        message.warning('No preceding user message found')
        return
      }

      let precedingUserMsg = null
      for (let i = currentIndex - 1; i >= 0; i--) {
        if (sorted[i].role === 'user') {
          precedingUserMsg = sorted[i]
          break
        }
      }

      if (!precedingUserMsg) {
        message.warning('No preceding user message found')
        return
      }

      const userText = (() => {
        for (const content of precedingUserMsg.contents) {
          const data = content.content as any
          if (data?.type === 'text' && typeof data.text === 'string') {
            return data.text
          }
        }
        return ''
      })()

      if (!userText) {
        message.warning('Cannot regenerate: preceding message has no text')
        return
      }

      Stores.Chat.__state.TextStore.setText(userText)
      // Mark this as an assistant-level fork so computeForkPoints anchors the
      // navigator at the assistant bubble on both parent and child branches.
      Stores.Chat.__state.BranchingStore.setPendingBranchForkLevel('assistant')
      // Branch from the preceding user message. The backend will clone everything
      // before that user message, then sendMessage adds the new user message —
      // no duplicate user message this way.
      Stores.Chat.__state.BranchingStore.setPendingBranchFromMessage(
        precedingUserMsg.id,
      )

      // Trim the user message and everything after it so the UI shows a clean
      // state during streaming (no layout shift when the new branch loads)
      await Stores.Chat.__state.BranchingStore.trimMessagesToForkPoint(
        precedingUserMsg.id,
      )

      await Stores.Chat.sendMessage()
    } catch (err: any) {
      message.error(err?.message || 'Regenerate failed')
    } finally {
      setIsRegenerating(false)
    }
  }

  return (
    <Space
      size={2}
      className="opacity-0 group-hover:opacity-100 transition-opacity"
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
