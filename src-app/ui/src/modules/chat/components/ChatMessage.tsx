import { memo } from 'react'
import { Avatar, theme } from 'antd'
import { UserOutlined } from '@ant-design/icons'
import type { MessageWithContent } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ContentRenderer } from '@/modules/chat/components/ContentRenderer'
import { MessageContext } from '@/modules/chat/core/MessageContext'
import { InlineEditor } from '@/modules/chat/extensions/branching/components/InlineEditor'

export const ChatMessage = memo(function ChatMessage({
  message,
}: {
  message: MessageWithContent
}) {
  const isUser = message.role === 'user'
  const { token } = theme.useToken()

  // Declaratively check if this message bubble is open for inline editing.
  // BranchingStore drives the UI — no local state needed here.
  const editingMessageId = Stores.Chat.BranchingStore?.editingMessageId
  const isEditing = editingMessageId === message.id

  // Check if message has any content to render
  if (!message.contents || message.contents.length === 0) {
    return null
  }

  return (
    <div className={'w-full flex flex-col overflow-visible group'} data-testid="chat-message" data-role={message.role}>
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
            {isEditing ? (
              /* Inline editor replaces the content area while editing */
              <InlineEditor />
            ) : (
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
            )}
          </div>
        </div>
      </div>

      {/* Extension slots rendered outside the bubble: navigator (always visible) then actions (hover) */}
      {!isEditing && (
        <MessageContext.Provider value={message}>
          <div className="flex flex-row items-center gap-1 mt-1">
            <ExtensionSlot name="message_item_suffix" />
            <ExtensionSlot name="message_actions" />
          </div>
        </MessageContext.Provider>
      )}
    </div>
  )
})
