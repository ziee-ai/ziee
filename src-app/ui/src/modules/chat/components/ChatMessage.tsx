import { memo } from 'react'
import { Avatar, theme } from 'antd'
import { UserOutlined } from '@ant-design/icons'
import type { MessageWithContent } from '@/api-client/types'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'
import { MessageContext } from '@/modules/chat/core/MessageContext'
import { BranchNavigator } from '@/modules/chat/components/BranchNavigator'
import { MessageActions } from '@/modules/chat/components/MessageActions'

export const ChatMessage = memo(function ChatMessage({
  message,
}: {
  message: MessageWithContent
}) {
  const isUser = message.role === 'user'
  const { token } = theme.useToken()

  // Check if message has any content to render
  if (!message.contents || message.contents.length === 0) {
    return null
  }

  // Render blocks in their authoritative backend order. Sort by
  // `sequence_order` (a copy — never mutate the store's array during
  // render), NOT `created_at`: blocks written in one DB transaction can
  // share a timestamp, and streaming-injected blocks carry monotonic
  // sequence_order. This keeps tool_use → tool_result(files) → text in
  // the right places.
  const sortedContents = [...message.contents].sort(
    (a, b) => a.sequence_order - b.sequence_order,
  )

  // For user messages, file attachments lift OUT of the text bubble and render
  // as a horizontal, wrapping row ABOVE it (outside the bordered box), instead
  // of stacking vertically inside it. Assistant messages keep every block in
  // the body (which has no bubble border anyway), so they're unchanged.
  const attachmentBlocks = isUser
    ? sortedContents.filter(c => c.content_type === 'file_attachment')
    : []
  const bubbleBlocks = isUser
    ? sortedContents.filter(c => c.content_type !== 'file_attachment')
    : sortedContents

  return (
    <div
      className={'w-full flex flex-col overflow-visible group'}
      data-testid="chat-message"
      data-role={message.role}
      data-message-id={message.id}
    >
      {/* User attachments: horizontal, wrapping row above the bubble. */}
      {attachmentBlocks.length > 0 && (
        <div
          className={'flex flex-wrap gap-2 mb-2'}
          data-testid="message-attachments"
        >
          {attachmentBlocks.map((content, index) => (
            <ContentRenderer
              key={`${content.id || `att-${index}`}`}
              content={content}
              isUser={isUser}
            />
          ))}
        </div>
      )}

      {/* Text bubble — only when there is non-attachment content. A files-only
          message has no text block (the text extension emits none for empty
          input), so bubbleBlocks is empty and no empty bubble renders. */}
      {bubbleBlocks.length > 0 && (
        <div
          key={message.id}
          className={`flex gap-2 rounded-lg relative min-w-36 flex-col`}
          style={{
            backgroundColor: isUser ? token.colorBgMask : 'transparent',
            border: isUser ? `1px solid ${token.colorBorderSecondary}` : 'none',
            width: isUser ? 'fit-content' : '100%',
            padding: isUser ? '8px 8px' : '0px',
          }}
        >
          <div className={'flex items-start gap-2 w-full relative'}>
            <div className={`flex ${!isUser ? 'hidden' : ''}`}>
              <Avatar size={24} icon={<UserOutlined />} />
            </div>

            {/* Message content */}
            <div
              className={`${isUser ? '!pt-0.5' : ''} flex flex-1 -mt-[2px] w-full overflow-x-hidden flex-col`}
            >
              <div className={'w-full flex flex-col gap-2'}>
                {bubbleBlocks.map((content, index) => (
                  <ContentRenderer
                    key={`${content.id || index}`}
                    content={content}
                    isUser={isUser}
                  />
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Core components + extension slots rendered outside the bubble */}
      <MessageContext.Provider value={message}>
        {/* Generic below-the-bubble extension point. Tool-returned files now
            render inline at their tool_result block (see the file extension's
            `tool_result` content renderer), so nothing registers here today. */}
        <ExtensionSlot name="message_footer" />
        <div className="flex flex-row items-center gap-1 mt-1">
          <BranchNavigator />
          <MessageActions />
          {/* Extensions can register additional message actions here */}
          <ExtensionSlot name="message_actions" />
        </div>
      </MessageContext.Provider>
    </div>
  )
})
