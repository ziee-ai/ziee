import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { Stores } from '@/core/stores'

export default function NewChatPage() {
  const navigate = useNavigate()

  useEffect(() => {
    Stores.Chat.reset()

    const unsubscribe = Stores.EventBus.on(
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
    <main className="flex flex-col h-full items-center justify-center p-8">
      <div className="w-full max-w-3xl">
        {/* Welcome message */}
        <div className="text-center mb-12">
          <Title level={2}>How can I help you today?</Title>
          <Text type="secondary" className="text-lg">
            Start a new conversation by typing a message below
          </Text>
        </div>

        {/* Chat input */}
        <div className="w-full">
          <ChatInput />
        </div>
      </div>
    </main>
  )
}
