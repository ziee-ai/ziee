import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import { Alert, Button, ErrorState, Tooltip } from '@/components/ui'
import { Search as SearchIcon } from 'lucide-react'
import { Loading } from '@/core/components/Loading'
import { MessageList } from '@/modules/chat/components/MessageList'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { TitleEditor } from '@/modules/chat/components/TitleEditor'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { ChatRightPanel } from '@/modules/chat/core/components/ChatRightPanel'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Stores } from '@/core'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { DivScrollY } from '@/components/common/DivScrollY'
import { cn } from '@/lib/utils'
import { ConversationFindBar } from '@/modules/chat/components/ConversationFindBar'
import { ConversationFindContext } from '@/modules/chat/components/ConversationFindContext'
import { JumpToLatestButton } from '@/modules/chat/components/JumpToLatestButton'

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
  // Drive the right panel's drawer-vs-side-panel by the conversation area's OWN
  // width (page size, sidebar-aware), not the window — so with the sidebar open
  // on a wide window the (now narrow) page still gets the overlay drawer.
  const mainAreaRef = useRef<HTMLDivElement>(null)
  const [rightPanelNarrow, setRightPanelNarrow] = useState(false)
  // Conversation id whose initial bottom-jump we've already done.
  const initialScrollConvIdRef = useRef<string | null>(null)

  // In-conversation find (ITEM-1) + jump-to-latest visibility (ITEM-2).
  const [findOpen, setFindOpen] = useState(false)
  // Restore focus to the toggle when the find bar closes (it unmounts, so its
  // focused input would otherwise drop focus to <body>).
  const findToggleRef = useRef<HTMLButtonElement>(null)
  const closeFind = () => {
    setFindOpen(false)
    findToggleRef.current?.focus()
  }
  const [activeMatchId, setActiveMatchId] = useState<string | null>(null)
  // Mirror of isAtBottomRef surfaced to render so the jump-to-latest button can
  // show/hide. The ref stays the source of truth for the scroll effects.
  const [atBottom, setAtBottom] = useState(true)

  // Re-attach when the conversation becomes available: on first mount the
  // Loading / ErrorState early-returns render NO sentinel, so an empty-dep
  // effect would bail once and never observe. Keying on the loaded conversation
  // id re-runs the effect when the main view (and the sentinel) actually mounts.
  useEffect(() => {
    const sentinel = messagesEndRef.current
    if (!sentinel) return
    const observer = new IntersectionObserver(
      entries => {
        const intersecting = entries[0]?.isIntersecting ?? false
        isAtBottomRef.current = intersecting
        setAtBottom(intersecting)
      },
      { root: null, threshold: 0 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [conversation?.id])

  // Measure the conversation area's width for the right-panel drawer decision.
  // Keyed on the loaded conversation id for the SAME reason as the sentinel
  // observer above: the main area only mounts once the Loading/Error early
  // returns clear, so an empty-dep effect would bail with a null ref and the
  // width would stay at its (narrow) initial value.
  useEffect(() => {
    const el = mainAreaRef.current
    if (!el) return
    const measure = () =>
      setRightPanelNarrow(el.getBoundingClientRect().width <= 640)
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [conversation?.id])

  // Cmd/Ctrl-F opens the in-conversation find bar, overriding the browser's
  // native find (our find covers the same rendered message content — DEC-5).
  // Only when a conversation is actually loaded — otherwise the find bar isn't
  // rendered (Loading / ErrorState early returns), so we must NOT swallow native
  // find and strand the user with neither.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === 'f') {
        if (!Stores.Chat.$.conversation) return
        e.preventDefault()
        setFindOpen(true)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  const findContextValue = useMemo(() => ({ activeMatchId }), [activeMatchId])

  const jumpToLatest = () =>
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })

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

  // No conversation to show. A load FAILURE (the store set `error`) is a
  // transient/permission problem → offer a persistent retry, not the misleading
  // "deleted" copy. A clean miss (no error) is a genuine not-found.
  if (!loading && !conversation) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-8">
        {error ? (
          <ErrorState
            resource="conversation"
            description="This conversation couldn't be loaded. Check your connection and try again."
            details={error}
            onRetry={() =>
              conversationId && Stores.Chat.loadConversation(conversationId)
            }
            className="max-w-md"
            data-testid="chat-conversation-error"
          />
        ) : (
          <Alert
            data-testid="chat-conversation-not-found-alert"
            tone="error"
            title="Conversation not found"
            description="This conversation may have been deleted or you don't have access to it."
          />
        )}
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
            {/* Find-in-conversation toggle (ITEM-1). Also openable via
                Cmd/Ctrl-F. */}
            <Tooltip content="Find in conversation">
              <Button
                ref={findToggleRef}
                data-testid="conversation-find-toggle-btn"
                variant={findOpen ? 'default' : 'ghost'}
                size="icon"
                icon={<SearchIcon />}
                aria-label="Find in conversation"
                aria-pressed={findOpen}
                onClick={() => (findOpen ? closeFind() : setFindOpen(true))}
              />
            </Tooltip>
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
      <div ref={mainAreaRef} className={cn('flex flex-1 min-h-0', nativeScroll ? '' : 'overflow-hidden')}>
        {/* Chat column. `relative` anchors the floating find bar (ITEM-1) and
            jump-to-latest button (ITEM-2). */}
        <div className={cn('relative flex flex-col flex-1 min-w-0', nativeScroll ? '' : 'overflow-hidden')}>
          {/* Floating find bar, top-right of the chat column. */}
          <div className="pointer-events-none absolute end-3 top-3 z-30 flex justify-end">
            <div className="pointer-events-auto">
              <ConversationFindBar
                open={findOpen}
                onClose={closeFind}
                onActiveMatchChange={setActiveMatchId}
              />
            </div>
          </div>
          {/* Pinned conversation-context chrome (the "In project" chip and any
              mode/model indicators registered into message_list_header). It's a
              SIBLING above the message scroll container — never a descendant of
              it — so it can't scroll out of view (K1/K4). Desktop: only the inner
              list scrolls, so normal flow already pins this. Mobile (native
              document-scroll): stick it to the top so it survives scroll-to-
              bottom. Renders nothing (zero height) when the conversation is
              unfiled, so there's no empty bar. */}
          <div
            className={cn(
              'w-full max-w-4xl mx-auto',
              nativeScroll ? 'sticky top-0 z-20 bg-background' : '',
            )}
            data-testid="conversation-context-chrome"
          >
            <ExtensionSlot name="message_list_header" />
          </div>
          {/* Desktop: overlay scroll (DivScrollY / OverlayScrollbars) so the
              message history matches every other scroll surface in the app
              instead of a heavy native scrollbar. `nativeFlow` keeps the mobile
              window-scroll path (sticky composer, iOS toolbar collapse)
              unchanged — it renders a plain flow container when nativeScroll is
              active, exactly like the previous conditional did. The
              messagesEnd sentinel + scrollIntoView + isAtBottom observer all
              keep working: OverlayScrollbars scrolls a real viewport element. The
              context chrome above stays a SIBLING of this scroller (K1/K4). */}
          <DivScrollY nativeFlow className="flex-1">
            {/* pb-8 matches the composer's h-8 fade so the last message can scroll
                clear of it at rest (fully readable, not dissolved). */}
            <div className="w-full max-w-4xl mx-auto px-4 pt-4 pb-8">
              <ConversationFindContext.Provider value={findContextValue}>
                <MessageList />
              </ConversationFindContext.Provider>
              <div ref={messagesEndRef} />
            </div>
          </DivScrollY>
          {/* Composer: pinned. Native mode → position:sticky at the viewport
              bottom (with home-indicator safe-area) so messages document-scroll
              underneath; desktop → normal flow at the column bottom. Made a
              positioning context (sticky in native, relative on desktop) so the
              jump-to-latest button can anchor to its TOP edge. */}
          <div
            className={cn(
              // pt-0: no gap above the input — the fade below stands in for it.
              'w-full max-w-4xl mx-auto p-4 pt-0',
              nativeScroll ? 'sticky bottom-0 z-10 bg-background' : 'relative',
            )}
            style={
              nativeScroll
                ? { paddingBottom: 'calc(env(safe-area-inset-bottom, 0px) + 16px)' }
                : undefined
            }
          >
            {/* Gradient fade above the composer: the tail of the message history
                dissolves into the surface (bg-card) as it scrolls up, instead of
                hard-cutting at the input's top edge. The message list carries a
                matching bottom pad so the last message clears this at rest. */}
            <div className="pointer-events-none absolute inset-x-0 bottom-full h-8 bg-gradient-to-t from-card to-transparent" />
            {/* Jump-to-latest: floats just ABOVE the composer (bottom-full anchors
                it to the composer's top edge, so it clears the input regardless of
                the input's height). Shown only when scrolled up (ITEM-2). */}
            <div className="pointer-events-none absolute inset-x-0 bottom-full mb-3 z-20 flex justify-center">
              <div className="pointer-events-auto">
                <JumpToLatestButton visible={!atBottom} onClick={jumpToLatest} />
              </div>
            </div>
            <ChatInput />
          </div>
        </div>

        {/* Right sidebar panel */}
        <ChatRightPanel narrow={rightPanelNarrow} />
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
