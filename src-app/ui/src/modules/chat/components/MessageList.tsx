import { Typography, Spin, Empty } from 'antd'
import { UserOutlined, RobotOutlined } from '@ant-design/icons'
import type { MessageWithContent } from '@/api-client/types'

const { Text } = Typography

interface MessageListProps {
  messages: Map<string, MessageWithContent>
  loading?: boolean
  isStreaming?: boolean
}

export function MessageList({ messages, loading = false, isStreaming = false }: MessageListProps) {
  if (loading && messages.size === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spin size="large" />
      </div>
    )
  }

  if (messages.size === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <Empty description="No messages yet. Start the conversation!" />
      </div>
    )
  }

  // Convert Map to array for rendering
  const messagesArray = Array.from(messages.values())

  return (
    <div className="flex flex-col gap-4 pb-4" data-testid="chat-messages">
      {messagesArray.map((message) => {
        const isUser = message.role === 'user'
        const textContent = message.contents.find(c => c.content_type === 'text')
        const text = textContent?.content?.text || ''

        return (
          <div
            key={message.id}
            className={`flex gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}
            data-testid="chat-message"
            data-role={message.role}
          >
            {/* Avatar */}
            <div className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center ${
              isUser ? 'bg-blue-500' : 'bg-gray-500'
            }`}>
              {isUser ? (
                <UserOutlined className="text-white" />
              ) : (
                <RobotOutlined className="text-white" />
              )}
            </div>

            {/* Message bubble */}
            <div
              className={`max-w-[70%] rounded-lg px-4 py-3 ${
                isUser
                  ? 'bg-blue-500 text-white'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
              }`}
            >
              <Text className={isUser ? 'text-white' : ''} style={{ whiteSpace: 'pre-wrap' }}>
                {text}
              </Text>
            </div>
          </div>
        )
      })}

      {/* Streaming indicator */}
      {isStreaming && (
        <div className="flex gap-3">
          <div className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-gray-500">
            <RobotOutlined className="text-white" />
          </div>
          <div className="bg-gray-100 dark:bg-gray-800 rounded-lg px-4 py-3">
            <Spin size="small" />
          </div>
        </div>
      )}
    </div>
  )
}
