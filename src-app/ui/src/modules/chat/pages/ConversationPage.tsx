import { useEffect, useLayoutEffect, useMemo, useRef } from 'react'
import { useParams } from 'react-router-dom'
import { Alert } from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { MessageList } from '@/modules/chat/components/MessageList'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Stores } from '@/core'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { cn } from '@/lib/utils'

export default function ConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()

  const { conversation, messages, loading, error } = Stores.Chat
  // Native document-scroll on mobile: the message history scrolls the WINDOW
  // (iOS toolbar collapses as you scroll up) while the composer stays pinned via
  // position:sticky. Desktop keeps the fixed inner-scroll shell. The right panel
  // is a fixed overlay on mobile, so the chat column is effectively full-width.
  useNativeScroll(true)
  const { nativeScroll } = Stores.AppLayout

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
  // Conversation id whose initial bottom-jump we've already done.
  const initialScrollConvIdRef = useRef<string | null>(null)

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

  // Initial load: jump to the bottom INSTANTLY (before paint), once per
  // conversation. An animated scroll-through would drag the viewport past
  // every message and trigger lazy-loading of all inline file previews — the
  // instant jump means off-screen previews never enter the viewport, so they
  // stay un-fetched until scrolled to. useLayoutEffect runs before paint so
  // the first frame is already at the bottom.
  //
  // Gate on the STORE's loaded conversation matching the URL, not just
  // `messages.size`: on an in-app A→B switch the URL param flips to B before
  // `loadConversation` runs, so for one render `conversationId===B` while the
  // store still holds A's `conversation`/`messages`. Latching then would
  // consume the jump against A's stale content and leave B to an animated
  // smooth-scroll-through (defeating lazy-loading). Requiring
  // `conversation?.id === conversationId` makes the latch wait for B's data.
  useLayoutEffect(() => {
    if (!conversationId) return
    if (
      conversation?.id === conversationId &&
      messages.size > 0 &&
      initialScrollConvIdRef.current !== conversationId
    ) {
      initialScrollConvIdRef.current = conversationId
      messagesEndRef.current?.scrollIntoView({ behavior: 'auto' })
    }
  }, [conversationId, conversation, messages])

  // Subsequent message changes (e.g. streaming deltas): smooth-follow, but
  // only when the loaded conversation matches the URL, the initial jump for it
  // has happened, and the user is already at the bottom. The conversation gate
  // stops a smooth animation from firing during the stale A→B switch window.
  useEffect(() => {
    if (
      conversation?.id === conversationId &&
      initialScrollConvIdRef.current === conversationId &&
      isAtBottomRef.current
    ) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages, conversationId, conversation])

  // Loading state
  if (loading && !conversation) {
    return (
      <Loading />
    )
  }

  // Error state
  if (!loading && !conversation) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-8">
        <Alert
          data-testid="chat-conversation-not-found-alert"
          tone="error"
          title="Conversation not found"
          description="This conversation may have been deleted or you don't have access to it."
        />
      </div>
    )
  }

  return (
    <div className={cn('flex flex-col', nativeScroll ? 'min-h-dvh' : 'h-full')}>
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
          <Alert data-testid="chat-conversation-error-alert" tone="error" title={error} onClose={Stores.Chat.clearError} closeLabel="Close" />
        </div>
      )}

      {/* Main area: chat column + right panel */}
      <div className={cn('flex flex-1 min-h-0', nativeScroll ? '' : 'overflow-hidden')}>
        {/* Chat column */}
        <div className={cn('flex flex-col flex-1 min-w-0', nativeScroll ? '' : 'overflow-hidden')}>
          <div className={cn('flex-1', nativeScroll ? '' : 'overflow-y-auto')}>
            <div className="w-full max-w-4xl mx-auto px-4 pt-4">
              <MessageList />
              <div ref={messagesEndRef} />
            </div>
          </div>
          {/* Composer: pinned. Native mode → position:sticky at the viewport
              bottom (with home-indicator safe-area) so messages document-scroll
              underneath; desktop → normal flow at the column bottom. */}
          <div
            className={cn(
              'w-full max-w-4xl mx-auto p-4 border-t border-border',
              nativeScroll ? 'sticky bottom-0 z-10 bg-background' : '',
            )}
            style={
              nativeScroll
                ? { paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 16px)' }
                : undefined
            }
          >
            <ChatInput />
          </div>
        </div>

        {/* Right sidebar panel */}
        <ChatRightPanel />
      </div>
    </div>
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
