import { useEffect, useRef } from 'react'
import { useParams, useLocation, useNavigate } from 'react-router-dom'
import { Spin, Alert } from 'antd'
import { useChatStore } from '../stores/Chat.store'
import { MessageList } from '../components/MessageList'
import { ChatInput } from '../components/ChatInput'
import { TitleEditor } from '../components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  const location = useLocation()
  const navigate = useNavigate()
  const pendingMessageSent = useRef(false)

  const {
    conversation,
    messages,
    loading,
    sending,
    isStreaming,
    error,
    loadConversation,
    loadMessages,
    sendMessage,
    updateConversation,
    clearError,
    reset
  } = useChatStore()

  // Load conversation and messages on mount or when ID changes
  useEffect(() => {
    if (conversationId) {
      reset()
      loadConversation(conversationId)
      loadMessages(conversationId)
    }
  }, [conversationId])

  // Handle pending message from NewChatPage
  useEffect(() => {
    const state = location.state as { pendingMessage?: string } | null

    if (
      state?.pendingMessage &&
      !pendingMessageSent.current &&
      conversation &&
      !loading
    ) {
      pendingMessageSent.current = true

      // Use conversation's model_id (should always be set when conversation is created)
      const defaultModelId = conversation.model_id || '00000000-0000-0000-0000-000000000000'

      sendMessage(state.pendingMessage, defaultModelId)

      // Clear the navigation state
      navigate(location.pathname, { replace: true, state: {} })
    }
  }, [conversation, loading, location])

  // Scroll to bottom when new messages arrive
  const messagesEndRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleSend = (content: string, modelId: string) => {
    if (conversation) {
      sendMessage(content, modelId)
    }
  }

  const handleTitleSave = async (title: string) => {
    await updateConversation({ title })
  }

  const handleBack = () => {
    navigate('/chat')
  }

  // Loading state
  if (loading && !conversation) {
    return (
      <div className="flex items-center justify-center h-full">
        <Spin size="large" />
      </div>
    )
  }

  // Error state
  if (!loading && !conversation) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-8">
        <Alert
          type="error"
          message="Conversation not found"
          description="This conversation may have been deleted or you don't have access to it."
          showIcon
        />
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
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
          <Alert
            type="error"
            message={error}
            closable
            onClose={clearError}
          />
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
          onSend={handleSend}
          disabled={sending || isStreaming}
          loading={sending}
          placeholder="Type your message..."
          defaultModelId={conversation?.model_id}
        />
      </div>
    </div>
  )
}
