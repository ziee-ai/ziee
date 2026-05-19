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

  return (
    <div
      className={'w-full flex flex-col overflow-visible group'}
      data-testid="chat-message"
      data-role={message.role}
      data-message-id={message.id}
    >
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
          </div>
        </div>
      </div>

      {/* Core components + extension slots rendered outside the bubble */}
      <MessageContext.Provider value={message}>
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
