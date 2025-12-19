import { memo } from 'react'
import { Avatar, theme } from 'antd'
import { UserOutlined } from '@ant-design/icons'
import type { MessageWithContent } from '@/api-client/types'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'

export const ChatMessage = memo(function ChatMessage({
  message,
}: {
  message: MessageWithContent
}) {
  const isUser = message.role === 'user'
  const { token } = theme.useToken()

  // Check if message has any content to render
  if (!message.contents || message.contents.length === 0) {
    return null // Skip rendering empty messages
  }

  return (
    <div className={'w-full flex flex-col overflow-visible'}>
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
              {message.contents
                .sort(
                  (a, b) =>
                    new Date(a.created_at).getTime() -
                    new Date(b.created_at).getTime(),
                )
                .map((content, index) => (
                  <ContentRenderer
                    key={`${content.id || index}`}
                    content={content}
                    isUser={isUser}
                  />
                ))}
            </div>

            {/* Extension slot: message actions (copy, edit, etc.) */}
            {/* TODO: Pass messageId when extension needs message-specific actions */}
            <ExtensionSlot name="message_actions" />
          </div>
        </div>
      </div>
    </div>
  )
})
