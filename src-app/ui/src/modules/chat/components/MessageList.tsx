import { Flex, Typography } from 'antd'
import { LoadingOutlined, MessageOutlined } from '@ant-design/icons'
import type { MessageWithContent } from '@/api-client/types'
import { ExtensionSlot } from '../core/extensions'
import { ChatMessage } from './ChatMessage'

const { Text } = Typography

interface MessageListProps {
  messages: Map<string, MessageWithContent>
  loading?: boolean
  isStreaming?: boolean
}

export function MessageList({
  messages,
  loading = false,
  isStreaming = false,
}: MessageListProps) {
  // Convert Map to array for rendering
  const messagesArray = Array.from(messages.values())

  if (!loading && messagesArray.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center py-20">
        <MessageOutlined className="text-5xl mb-4" />
        <Text className="text-lg">Start your conversation</Text>
      </div>
    )
  }

  return (
    <Flex className={'flex-col gap-1 w-full'} data-testid="chat-messages">
      {/* Extension slot: message list header */}
      <ExtensionSlot name="message_list_header" />

      {messagesArray.map(msg => (
        <ChatMessage key={msg.id} message={msg} />
      ))}

      {/* Streaming indicator */}
      {(loading || isStreaming) && (
        <div className={'w-full h-20 mt-3'}>
          <LoadingOutlined spin className={'text-xl'} />
        </div>
      )}

      {/* Extension slot: message list footer */}
      <ExtensionSlot name="message_list_footer" />
    </Flex>
  )
}
