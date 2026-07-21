import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Text, Title } from '@ziee/kit'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { Chat as ChatStore } from '@/modules/chat/core/stores/chatBridge'
import { EventBus } from '@ziee/framework/stores'

export default function NewChatPage() {
  const navigate = useNavigate()

  useEffect(() => {
    ChatStore.reset()

    const unsubscribe = EventBus.on(
      'conversation.created',
      event => {
        navigate(`/chat/${event.data.conversation.id}`)
      },
      'NewChatPage',
    )

    return () => {
      unsubscribe()
    }
  }, [navigate])

  return (
    <div className="flex flex-col h-full items-center justify-center p-4">
      <div className="w-full max-w-3xl">
        {/* Welcome message */}
        <div className="text-center mb-12">
          <Title level={2} data-testid="new-chat-greeting">How can I help you today?</Title>
          <Text type="secondary" className="text-lg">
            Start a new conversation by typing a message below
          </Text>
        </div>

        {/* Chat input */}
        <div className="w-full">
          <ChatInput />
        </div>
      </div>
    </div>
  )
}
