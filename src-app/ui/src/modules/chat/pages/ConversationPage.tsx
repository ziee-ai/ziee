import { useEffect, useRef } from 'react'
import { useParams, useLocation, useNavigate } from 'react-router-dom'
import { Typography, Spin, Button, Alert } from 'antd'
import { ArrowLeftOutlined } from '@ant-design/icons'
import { useChatStore } from '../stores/Chat.store'
import { MessageList } from '../components/MessageList'
import { ChatInput } from '../components/ChatInput'

const { Title } = Typography

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

      // Get first available model (simplified - in production you'd have model selection)
      // For now, we'll just use a hardcoded model ID or fetch from a default
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

  const handleSend = (content: string) => {
    if (conversation) {
      const defaultModelId = conversation.model_id || '00000000-0000-0000-0000-000000000000'
      sendMessage(content, defaultModelId)
    }
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
        <Button
          type="primary"
          onClick={() => navigate('/chat')}
          className="mt-4"
        >
          Start New Chat
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 p-4 border-b border-gray-200 dark:border-gray-700">
        <Button
          type="text"
          icon={<ArrowLeftOutlined />}
          onClick={() => navigate('/chat')}
        />
        <Title level={4} className="m-0">
          {conversation?.title || 'Untitled Conversation'}
        </Title>
      </div>

      {/* Error banner */}
      {error && (
        <Alert
          type="error"
          message={error}
          closable
          onClose={clearError}
          className="m-4"
        />
      )}

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto px-4 pt-4">
        <MessageList
          messages={messages}
          loading={loading}
          isStreaming={isStreaming}
        />
        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <div className="p-4 border-t border-gray-200 dark:border-gray-700">
        <ChatInput
          onSend={handleSend}
          disabled={sending || isStreaming}
          loading={sending}
          placeholder="Type your message..."
        />
      </div>
    </div>
  )
}
