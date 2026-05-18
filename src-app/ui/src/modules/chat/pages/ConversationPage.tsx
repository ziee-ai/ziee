import { useEffect, useRef } from 'react'
import { useParams } from 'react-router-dom'
import { Spin, Alert, theme } from 'antd'
import { MessageList } from '@/modules/chat/components/MessageList'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { Stores } from '@/core'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  const { token } = theme.useToken()

  const { conversation, messages, loading, error } = Stores.Chat

  // Load conversation and messages on mount or when ID changes
  useEffect(() => {
    if (conversationId) {
      Stores.Chat.loadConversation(conversationId)
    }
  }, [conversationId])

  // Scroll to bottom when new messages arrive
  const messagesEndRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

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
          title="Conversation not found"
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
          <TitleEditor />
        </div>
      </HeaderBarContainer>

      {/* Error banner */}
      {error && (
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <Alert type="error" title={error} closable onClose={Stores.Chat.clearError} />
        </div>
      )}

      {/* Main area: chat column + right panel */}
      <div className="flex flex-1 overflow-hidden min-h-0">
        {/* Chat column */}
        <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
          <div className="flex-1 overflow-y-auto">
            <div className="w-full max-w-4xl mx-auto px-4 pt-4">
              <MessageList />
              <div ref={messagesEndRef} />
            </div>
          </div>
          <div className="w-full max-w-4xl mx-auto p-4" style={{ borderTop: `1px solid ${token.colorBorderSecondary}` }}>
            <ChatInput />
          </div>
        </div>

        {/* Right sidebar panel */}
        <ChatRightPanel />
      </div>
    </main>
  )
}
