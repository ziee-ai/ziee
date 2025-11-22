import { useNavigate } from 'react-router-dom'
import { Typography, message as antMessage } from 'antd'
import { ChatInput } from '../components/ChatInput'
import { ApiClient } from '@/api-client'
import { useState } from 'react'

const { Title, Text } = Typography

export default function NewChatPage() {
  const navigate = useNavigate()
  const [creating, setCreating] = useState(false)

  const handleFirstMessage = async (content: string) => {
    setCreating(true)
    try {
      // Create new conversation
      const conversation = await ApiClient.Conversation.create({
        model_id: undefined, // Optional: will use default
        title: undefined, // Will be auto-generated
      })

      // Navigate to conversation page with pending message
      // We'll pass the message content via state
      navigate(`/chat/${conversation.id}`, {
        state: { pendingMessage: content }
      })
    } catch (error: any) {
      console.error('Failed to create conversation:', error)
      antMessage.error(error.message || 'Failed to create conversation')
      setCreating(false)
    }
  }

  return (
    <div className="flex flex-col h-full items-center justify-center p-8">
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
          <ChatInput
            onSend={handleFirstMessage}
            disabled={creating}
            loading={creating}
            placeholder="Type your message to start a conversation..."
          />
        </div>
      </div>
    </div>
  )
}
