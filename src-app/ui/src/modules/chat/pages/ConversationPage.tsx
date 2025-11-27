import { useEffect, useRef } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { Spin, Alert } from 'antd'
import { MessageList } from '../components/MessageList'
import { ChatInput } from '../components/ChatInput'
import { TitleEditor } from '../components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { Stores } from '@/core'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  const navigate = useNavigate()

  const {
    conversation,
    messages,
    loading,
    isStreaming,
    error,
  } = Stores.Chat

  // Load conversation and messages on mount or when ID changes
  useEffect(() => {
    if (conversationId) {
      // Stores.Chat.reset()
      Stores.Chat.loadConversation(conversationId)
      Stores.Chat.loadMessages(conversationId)
    }
  }, [conversationId])

  // Scroll to bottom when new messages arrive
  const messagesEndRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleTitleSave = async (title: string) => {
    await Stores.Chat.updateConversation({ title })
  }

  const handleBack = () => {
    navigate('/chat')
  }

  // Loading state
  if (loading && !conversation) {
    return (
      <main className="flex items-center justify-center h-full">
        <Spin size="large" />
      </main>
    )
  }

  // Error state
  if (!loading && !conversation) {
    return (
      <main className="flex flex-col items-center justify-center h-full p-8">
        <Alert
          type="error"
          message="Conversation not found"
          description="This conversation may have been deleted or you don't have access to it."
          showIcon
        />
      </main>
    )
  }

  return (
    <main className="flex flex-col h-full">
      {/* Header */}
      <HeaderBarContainer>
        <div className="w-full max-w-4xl mx-auto flex items-center">
          <TitleEditor
            conversation={conversation}
            onSave={handleTitleSave}
            onBack={handleBack}
          />
        </div>
      </HeaderBarContainer>

      {/* Error banner */}
      {error && (
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <Alert type="error" message={error} closable onClose={Stores.Chat.clearError} />
        </div>
      )}

      {/* Messages area - centered with max-width */}
      <div className="flex-1 overflow-y-auto">
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <MessageList
            messages={messages}
            loading={loading}
            isStreaming={isStreaming}
          />
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Input area - centered with max-width */}
      <div className="w-full max-w-4xl mx-auto p-4 border-t border-gray-200 dark:border-gray-700">
        <ChatInput
          placeholder="Type your message..."
          defaultModelId={conversation?.model_id}
        />
      </div>
    </main>
  )
}
