import { useEffect, useMemo, useRef } from 'react'
import { useParams } from 'react-router-dom'
import { Spin, Alert, theme } from 'antd'
import { MessageList } from '@/modules/chat/components/MessageList'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Stores } from '@/core'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  const { token } = theme.useToken()

  const { conversation, messages, loading, error } = Stores.Chat

  // Load conversation and messages on mount or when ID changes.
  useEffect(() => {
    if (conversationId) {
      Stores.Chat.loadConversation(conversationId)
    }
  }, [conversationId])

  // Auto-scroll to bottom on new messages, but ONLY if the user is
  // already near the bottom. Previously this scrolled unconditionally
  // on every message-map change, yanking the user back to the bottom
  // whenever they scrolled up to read history mid-stream. (audit 04 HIGH-3)
  //
  // Implementation: an IntersectionObserver watches the messagesEnd
  // sentinel. When it intersects the viewport (i.e., the user is at
  // the bottom), we set isAtBottomRef = true; when it leaves, false.
  // The scroll effect only fires when isAtBottomRef is true.
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const isAtBottomRef = useRef(true)

  useEffect(() => {
    const sentinel = messagesEndRef.current
    if (!sentinel) return
    const observer = new IntersectionObserver(
      entries => {
        isAtBottomRef.current = entries[0]?.isIntersecting ?? false
      },
      { root: null, threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (isAtBottomRef.current) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
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
      {/* Header — full width, matches the rest of the app's header bars
          (project page, settings, etc.). TitleEditor at the left,
          slot consumers at the right. */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <div className="flex items-center min-w-0 gap-2">
            <TitleEditor />
          </div>
          <div className="flex items-center gap-1">
            {/* Decoupled chip injection point — other modules register
                header decorations into `chatConversationHeaderTrailing`
                without chat compiling against them. */}
            <ConversationHeaderTrailingSlot />
          </div>
        </div>
      </HeaderBarContainer>

      {/* Error banner */}
      {error && (
        <div className="w-full max-w-4xl mx-auto px-4 pt-4">
          <Alert type="error" title={error} closable={{ onClose: Stores.Chat.clearError }} />
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

/**
 * Consumes the `chatConversationHeaderTrailing` slot. Other modules
 * register header decorations here; chat doesn't compile against
 * any of them.
 */
function ConversationHeaderTrailingSlot() {
  const { slots } = Stores.ModuleSystem
  const rawItems = slots.get('chatConversationHeaderTrailing')
  // Memoize the sorted copy — the underlying slot array is stable
  // (mutates only on module-registration changes, which don't happen
  // post-bootstrap), so this collapses to a one-time sort.
  const items = useMemo(
    () =>
      (rawItems || [])
        .slice()
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    [rawItems],
  )
  return (
    <>
      {items.map(item => (
        <LazyComponentRenderer key={item.id} component={item.component} />
      ))}
    </>
  )
}
