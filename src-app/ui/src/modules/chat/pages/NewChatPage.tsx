import { useNavigate } from 'react-router-dom'
import { Typography, message as antMessage } from 'antd'
import { ChatInput } from '../components/ChatInput'
import { useChatStore } from '../stores/Chat.store'

const { Title, Text } = Typography

export default function NewChatPage() {
  const navigate = useNavigate()
  const { createConversation, loading } = useChatStore()

  const handleFirstMessage = async (content: string, modelId: string) => {
    try {
      // Create new conversation with the selected model
      const conversation = await createConversation(modelId)

      // Navigate to conversation page with pending message
      // We'll pass the message content via state
      navigate(`/chat/${conversation.id}`, {
        state: { pendingMessage: content }
      })
    } catch (error: any) {
      console.error('Failed to create conversation:', error)
      antMessage.error(error.message || 'Failed to create conversation')
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
            disabled={loading}
            loading={loading}
            placeholder="Type your message to start a conversation..."
          />
        </div>
      </div>
    </div>
  )
}
