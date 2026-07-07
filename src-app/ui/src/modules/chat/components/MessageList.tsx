import { Flex } from '@/components/ui'
import { Text } from '@/components/ui'
import { Loader2, MessageSquare } from 'lucide-react'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatMessage } from '@/modules/chat/components/ChatMessage'
import { Stores } from '@/core/stores'

/**
 * MessageList Component
 * Self-contained component that accesses messages and loading state from store
 */
export function MessageList() {
  // Get data from store
  const { messages, loading, isStreaming } = Stores.Chat

  // Convert Map to array for rendering
  const messagesArray = Array.from(messages.values())

  if (!loading && messagesArray.length === 0) {
    return (
      <Flex className={'flex-col gap-1 w-full h-full'} data-testid="chat-messages">
        {/* The "In project" chip and other persistent context markers are NOT
            rendered here — they live in ConversationPage as PINNED chrome above
            the message scroll container so they never scroll out of view. */}
        <div className="flex flex-1 flex-col items-center justify-center text-center py-20">
          <MessageSquare className="text-5xl mb-4" />
          <Text className="text-lg">Start your conversation</Text>
        </div>
      </Flex>
    )
  }

  return (
    <Flex className={'flex-col gap-1 w-full'} data-testid="chat-messages">
      {/* The message_list_header slot (project chip / context) is rendered as
          pinned chrome in ConversationPage, above this scroll container. */}
      {messagesArray.map((msg, i) => (
        <ChatMessage
          key={msg.id}
          message={msg}
          // The streaming message is the last assistant message while a stream
          // is in flight — never collapse it (DEC-6).
          isStreaming={
            isStreaming &&
            i === messagesArray.length - 1 &&
            msg.role === 'assistant'
          }
        />
      ))}

      {/* Streaming indicator */}
      {(loading || isStreaming) && (
        <div className={'w-full h-20 mt-3'}>
          <Loader2 className={'text-xl animate-spin'} />
        </div>
      )}

      {/* Extension slot: message list footer */}
      <ExtensionSlot name="message_list_footer" />
    </Flex>
  )
}
